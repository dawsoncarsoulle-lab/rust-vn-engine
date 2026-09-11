//! Shared design resolution. Authoring data never changes during rendering.
use crate::*;
pub fn card_state_bindings(gallery:bool,filled:bool,locked:bool)->BTreeMap<String,String>{
    if gallery{[("gallery.locked",locked),("gallery.unlocked",!locked)].into_iter().map(|(k,v)|(k.into(),v.to_string())).collect()}
    else{[("save.empty",!filled),("save.filled",filled),("save.locked",locked),("save.unlocked",!locked)].into_iter().map(|(k,v)|(k.into(),v.to_string())).collect()}
}

#[derive(Clone,Copy,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]
pub enum ThemePreset {#[default] Sobre,Illustre,ScienceFiction}
#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
pub enum PageRole {Title,Pause,Save,Load,Settings,Gallery,History,Confirm,Dialogue,Choices,QuickActions}
impl PageRole {
    pub fn from_id(id:&str)->Option<Self>{Some(match id{"title"=>Self::Title,"pause"=>Self::Pause,"save"=>Self::Save,"load"=>Self::Load,"settings"=>Self::Settings,"gallery"=>Self::Gallery,"history"=>Self::History,"confirm"=>Self::Confirm,"dialogue"=>Self::Dialogue,"choices"=>Self::Choices,"quick_actions"=>Self::QuickActions,_=>return None})}
}
#[derive(Clone,Copy,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]
pub enum ImageFit {#[default] Contain,Cover,Stretch,NineSlice}
#[derive(Clone,Copy,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]
pub enum TextAlign{#[default] Left,Center,Right}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct ListOptions {pub template:Option<String>,pub columns:usize,pub rows:usize,pub slots:usize,pub order:ListOrder}
#[derive(Clone,Copy,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]
pub enum ListOrder{#[default] Rows,Columns}
impl Default for ListOptions{fn default()->Self{Self{template:None,columns:1,rows:5,slots:5,order:ListOrder::Rows}}}
impl ListOptions {
    pub fn content_height(&self,count:usize,size:[f32;2],gap:f32)->f32{(0..count).map(|index|{let r=self.card_rect(index,size,gap);r[1]+r[3]}).fold(0.0,f32::max)}
    /// Card bounds inside a list; shared by the authoring preview and game.
    pub fn card_rect(&self,index:usize,size:[f32;2],gap:f32)->[f32;4]{
        self.card_rect_with_navigation(index,size,gap,true)
    }
    pub fn card_rect_with_navigation(&self,index:usize,size:[f32;2],gap:f32,automatic:bool)->[f32;4]{
        let columns=self.columns.max(1);let rows=self.rows.max(1);
        let footer=if automatic&&self.slots>columns*rows{44.0}else{0.0};
        let w=((size[0]-gap*(columns-1) as f32)/columns as f32).max(1.0);
        let h=((size[1]-footer-gap*(rows-1) as f32)/rows as f32).max(1.0);
        let (column,row)=match self.order{ListOrder::Rows=>(index%columns,index/columns),ListOrder::Columns=>{let capacity=columns*rows;let local=index%capacity;(local/rows,local%rows+(index/capacity)*rows)}};
        [column as f32*(w+gap),row as f32*(h+gap),w,h]
    }
}
#[cfg(test)]mod list_order_tests{use super::*;#[test]fn column_order_changes_positions_not_slot_indices(){let mut list=ListOptions{columns:3,rows:2,slots:6,..Default::default()};let size=[900.0,600.0];assert_eq!(list.card_rect(1,size,0.0),[300.0,0.0,300.0,300.0]);list.order=ListOrder::Columns;for(i,(x,y))in[(0.0,0.0),(0.0,300.0),(300.0,0.0),(300.0,300.0),(600.0,0.0),(600.0,300.0)].into_iter().enumerate(){assert_eq!(list.card_rect(i,size,0.0),[x,y,300.0,300.0]);}assert_eq!(list.card_rect(6,size,0.0),[0.0,600.0,300.0,300.0]);assert_eq!(list.content_height(7,size,0.0),900.0);let legacy:ListOptions=serde_json::from_str(r#"{"columns":3,"rows":2,"slots":6}"#).unwrap();assert_eq!(legacy.order,ListOrder::Rows);}}
#[cfg(test)]mod auto_height_tests{use super::*;#[test]fn measured_text_moves_following_siblings_without_mutating_source(){let mut d=Document::defaults();let mut column=Element::new("column".into(),Kind::Vertical);column.rect=[100.0,80.0,500.0,800.0];column.layout_options.gap=10.0;let mut text=Element::new("text".into(),Kind::Text);text.rect=[0.0,0.0,500.0,40.0];text.layout_options.auto_height=true;let mut button=Element::new("button".into(),Kind::Button);button.rect=[0.0,0.0,500.0,60.0];column.children=vec![text,button];d.pages[0].elements=vec![column];let before=d.clone();for scale in [0.5,1.0,1.5]{let size=d.reference.map(|v|v*scale);let flat=d.layout_page_measured(0,size,&mut|_,width,s|{assert_eq!(width,500.0*scale);Some(160.0*s)});let text=flat.iter().find(|e|e.id=="text").unwrap();let button=flat.iter().find(|e|e.id=="button").unwrap();assert_eq!(text.rect[3],160.0*scale);assert_eq!(button.rect[1],250.0*scale);}assert_eq!(d,before);d.validate().unwrap();d.find_element_mut(0,"text").unwrap().anchors[3]=1.0;assert!(d.validate().is_err());}}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct LayoutOptions {pub padding:[f32;4],pub gap:f32,pub columns:usize,pub image_fit:ImageFit,pub slice:[f32;4],pub text_align:TextAlign,pub text_wrap:bool,pub auto_height:bool}
impl Default for LayoutOptions {fn default()->Self{Self{padding:[0.0;4],gap:12.0,columns:2,image_fit:ImageFit::Contain,slice:[0.0;4],text_align:TextAlign::Left,text_wrap:true,auto_height:false}}}
/// Destination rectangles and normalized source rectangles, shared by renderers.
pub fn image_quads(image:[f32;2],size:[f32;2],fit:ImageFit,border:[f32;4])->Vec<([f32;4],[f32;4])>{
    if image.iter().chain(size.iter()).any(|v|!v.is_finite()||*v<=0.0)||border.iter().any(|v|!v.is_finite()||*v<0.0){return vec![]}
    match fit{
        ImageFit::Contain=>{let s=(size[0]/image[0]).min(size[1]/image[1]);vec![([(size[0]-image[0]*s)*0.5,(size[1]-image[1]*s)*0.5,image[0]*s,image[1]*s],[0.0,0.0,1.0,1.0])]},
        ImageFit::Cover=>{let s=(size[0]/image[0]).max(size[1]/image[1]);let uv=[size[0]/(image[0]*s),size[1]/(image[1]*s)];vec![([0.0,0.0,size[0],size[1]],[(1.0-uv[0])*0.5,(1.0-uv[1])*0.5,uv[0],uv[1]])]},
        ImageFit::Stretch=>vec![([0.0,0.0,size[0],size[1]],[0.0,0.0,1.0,1.0])],
        ImageFit::NineSlice=>{let [left,top,right,bottom]=border;if left+right>=image[0]||top+bottom>=image[1]{return vec![]}
            let s=(size[0]/image[0]).min(size[1]/image[1]).min(1.0);let xs=[0.0,left*s,size[0]-right*s,size[0]];let ys=[0.0,top*s,size[1]-bottom*s,size[1]];let us=[0.0,left/image[0],1.0-right/image[0],1.0];let vs=[0.0,top/image[1],1.0-bottom/image[1],1.0];let mut result=vec![];
            for y in 0..3{for x in 0..3{if xs[x+1]>xs[x]&&ys[y+1]>ys[y]{result.push(([xs[x],ys[y],xs[x+1]-xs[x],ys[y+1]-ys[y]],[us[x],vs[y],us[x+1]-us[x],vs[y+1]-vs[y]]));}}}result},
    }
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct Shadow {pub offset:[f32;2],pub blur:f32,pub color:Color}
impl Default for Shadow{fn default()->Self{Self{offset:[6.0,6.0],blur:8.0,color:[0.0;4]}}}
impl Shadow{
    pub fn validate(&self)->Result<(),String>{if self.offset.iter().any(|v|!v.is_finite()||v.abs()>4096.0)||!self.blur.is_finite()||!(0.0..=256.0).contains(&self.blur)||self.color.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err(diagnostic!("Ombre invalide : décalage ±4096, diffusion 0 à 256 et couleur RGBA valide requis", "Invalid shadow: offset ±4096, blur 0 to 256 and a valid RGBA color are required").into());}Ok(())}
}
/// Identical soft-edge layers in the native game and the authoring canvas.
pub fn shadow_layers(rect:[f32;4],radius:f32,shadow:&Shadow)->Vec<([f32;4],f32,Color)>{
    if shadow.color[3]<=0.0{return vec![];}
    let count=if shadow.blur>0.0{8}else{1};let mut color=shadow.color;color[3]=1.0-(1.0-color[3]).powf(1.0/count as f32);
    (0..count).rev().map(|i|{let spread=shadow.blur*i as f32/count as f32;([rect[0]+shadow.offset[0]-spread,rect[1]+shadow.offset[1]-spread,rect[2]+2.0*spread,rect[3]+2.0*spread],radius+spread,color)}).collect()
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct ScrollbarStyle{pub track:Color,pub thumb:Color,pub hover:Color,pub pressed:Color,pub width:f32,pub radius:f32}
impl Default for ScrollbarStyle{fn default()->Self{Self{track:[0.05,0.06,0.07,0.65],thumb:[0.55,0.60,0.65,0.85],hover:[0.7,0.75,0.8,1.0],pressed:[0.85,0.9,0.95,1.0],width:12.0,radius:4.0}}}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct ScrollbarPatch{pub track:Option<Color>,pub thumb:Option<Color>,pub hover:Option<Color>,pub pressed:Option<Color>,pub width:Option<f32>,pub radius:Option<f32>}
impl ScrollbarPatch{
    pub fn from_style(s:&ScrollbarStyle)->Self{Self{track:Some(s.track),thumb:Some(s.thumb),hover:Some(s.hover),pressed:Some(s.pressed),width:Some(s.width),radius:Some(s.radius)}}
    pub fn apply(&self,s:&mut ScrollbarStyle){if let Some(v)=self.track{s.track=v}if let Some(v)=self.thumb{s.thumb=v}if let Some(v)=self.hover{s.hover=v}if let Some(v)=self.pressed{s.pressed=v}if let Some(v)=self.width{s.width=v}if let Some(v)=self.radius{s.radius=v}}
    pub fn validate(&self)->Result<(),String>{
        for c in [self.track,self.thumb,self.hover,self.pressed].into_iter().flatten(){if c.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err(diagnostic!("Couleur de barre de défilement invalide", "Invalid scrollbar color").into());}}
        if self.width.is_some_and(|v|!v.is_finite()||!(4.0..=64.0).contains(&v))||self.radius.is_some_and(|v|!v.is_finite()||!(0.0..=32.0).contains(&v)){return Err(diagnostic!("Barre de défilement : largeur de 4 à 64 px et arrondi de 0 à 32 px", "Scrollbar: width from 4 to 64 px and corner radius from 0 to 32 px").into());}Ok(())
    }
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct Appearance {
    pub radius:f32,
    pub border_width:f32,
    pub border_color:Color,
    pub focus:Option<Color>,
    pub selected:Option<Color>,
    pub opacity:f32,
    pub shadow:Shadow,
    pub scrollbar:ScrollbarStyle,
    pub text_states:TextStateColors,
    pub image_states:StateImages,
}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct TextStateColors{pub hover:Option<Color>,pub pressed:Option<Color>,pub disabled:Option<Color>,pub focus:Option<Color>,pub selected:Option<Color>}
impl TextStateColors{
    pub fn apply(&self,target:&mut Self){if self.hover.is_some(){target.hover=self.hover;}if self.pressed.is_some(){target.pressed=self.pressed;}if self.disabled.is_some(){target.disabled=self.disabled;}if self.focus.is_some(){target.focus=self.focus;}if self.selected.is_some(){target.selected=self.selected;}}
    pub fn resolve(&self,normal:Color,enabled:bool,pressed:bool,hover:bool,focus:bool,selected:bool)->Color{if !enabled{return self.disabled.unwrap_or(normal);}let idle=if selected{self.selected.unwrap_or(normal)}else{normal};if pressed{return self.pressed.or(self.hover).unwrap_or(idle);}if hover{return self.hover.unwrap_or(idle);}if focus{return self.focus.or(self.hover).unwrap_or(idle);}idle}
    pub fn opacity(&mut self,opacity:f32){for c in [&mut self.hover,&mut self.pressed,&mut self.disabled,&mut self.focus,&mut self.selected].into_iter().flatten(){c[3]*=opacity;}}
    pub fn validate(&self)->Result<(),String>{for c in [self.hover,self.pressed,self.disabled,self.focus,self.selected].into_iter().flatten(){if c.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err(diagnostic!("Couleur de texte interactive invalide", "Invalid interactive text color").into());}}Ok(())}
}
impl Default for Appearance{fn default()->Self{Self{radius:0.0,border_width:0.0,border_color:[0.0;4],focus:None,selected:None,opacity:1.0,shadow:Shadow::default(),scrollbar:ScrollbarStyle::default(),text_states:TextStateColors::default(),image_states:StateImages::default()}}}
impl Appearance {
    pub fn scale(&mut self,scale:f32){self.radius*=scale;self.border_width*=scale;self.shadow.offset=self.shadow.offset.map(|v|v*scale);self.shadow.blur*=scale;}
    pub fn validate(&self)->Result<(),String>{
        self.shadow.validate()?;
        self.text_states.validate()?;
        ScrollbarPatch::from_style(&self.scrollbar).validate()?;
        StylePatch{radius:Some(self.radius),border_width:Some(self.border_width),border_color:Some(self.border_color),focus:self.focus,selected:self.selected,opacity:Some(self.opacity),..Default::default()}.validate()
    }
}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct StylePatch {
    pub normal:Option<Color>,pub hover:Option<Color>,pub pressed:Option<Color>,pub disabled:Option<Color>,pub foreground:Option<Color>,
    pub font:Option<String>,pub font_size:Option<f32>,pub radius:Option<f32>,pub border_width:Option<f32>,pub border_color:Option<Color>,pub focus:Option<Color>,pub selected:Option<Color>,
    pub image:Option<ResourceOverride>,
    pub layout:Option<LayoutOptions>,
    pub opacity:Option<f32>,
    pub shadow:Option<Shadow>,
    pub scrollbar:ScrollbarPatch,
    pub text_states:TextStateColors,
    pub image_states:StateImages,
}
/// None on the patch means inherited; Clear explicitly removes an inherited image.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub enum ResourceOverride { Clear, Path(String) }
impl StylePatch {
    pub fn from_element(e:&Element)->Self{Self{normal:Some(e.normal),hover:Some(e.hover),pressed:Some(e.pressed),disabled:Some(e.disabled),foreground:Some(e.foreground),font:e.font.clone(),font_size:Some(e.font_size),radius:Some(e.appearance.radius),border_width:Some(e.appearance.border_width),border_color:Some(e.appearance.border_color),focus:e.appearance.focus,selected:e.appearance.selected,image:Some(e.asset.clone().map(ResourceOverride::Path).unwrap_or(ResourceOverride::Clear)),layout:None,opacity:Some(e.appearance.opacity),shadow:Some(e.appearance.shadow.clone()),scrollbar:ScrollbarPatch::from_style(&e.appearance.scrollbar),text_states:e.appearance.text_states.clone(),image_states:e.appearance.image_states.clone()}}
    pub fn apply(&self,e:&mut Element){
        self.scrollbar.apply(&mut e.appearance.scrollbar);
        self.text_states.apply(&mut e.appearance.text_states);
        self.image_states.apply(&mut e.appearance.image_states);
        if let Some(v)=&self.image{e.asset=match v{ResourceOverride::Clear=>None,ResourceOverride::Path(path)=>Some(path.clone())};}
        if let Some(v)=&self.shadow{e.appearance.shadow=v.clone();}if let Some(v)=&self.layout{e.layout_options=v.clone();}if let Some(v)=self.opacity{e.appearance.opacity=v;}
        if let Some(v)=self.normal{e.normal=v}if let Some(v)=self.hover{e.hover=v}if let Some(v)=self.pressed{e.pressed=v}if let Some(v)=self.disabled{e.disabled=v}if let Some(v)=self.foreground{e.foreground=v}
        if let Some(v)=&self.font{e.font=Some(v.clone())}if let Some(v)=self.font_size{e.font_size=v}
        if let Some(v)=self.radius{e.appearance.radius=v}if let Some(v)=self.border_width{e.appearance.border_width=v}if let Some(v)=self.border_color{e.appearance.border_color=v}if let Some(v)=self.focus{e.appearance.focus=Some(v)}if let Some(v)=self.selected{e.appearance.selected=Some(v)}
    }
    pub fn validate(&self)->Result<(),String>{
        self.scrollbar.validate()?;
        self.text_states.validate()?;
        if let Some(shadow)=&self.shadow{shadow.validate()?;}if let Some(l)=&self.layout{if l.padding.iter().chain(l.slice.iter()).chain(std::iter::once(&l.gap)).any(|v|!v.is_finite()||*v<0.0)||l.columns==0||l.columns>64{return Err(diagnostic!("Disposition locale invalide", "Invalid local layout").into())}}
        if self.opacity.is_some_and(|v|!v.is_finite()||!(0.0..=1.0).contains(&v)){return Err(diagnostic!("Opacité invalide", "Invalid opacity").into());}
        for c in [self.normal,self.hover,self.pressed,self.disabled,self.foreground,self.border_color,self.focus,self.selected].into_iter().flatten(){if c.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err(diagnostic!("Couleur de style invalide", "Invalid style color").into())}}
        for v in [self.font_size,self.radius,self.border_width].into_iter().flatten(){if !v.is_finite()||v<0.0{return Err(diagnostic!("Dimension de style invalide", "Invalid style dimension").into())}}
        if self.font_size==Some(0.0){return Err(diagnostic!("Taille de police nulle", "Font size must be greater than zero").into())}Ok(())
    }
}
/// Shared by the inspector's choices and document validation.
pub fn binding_compatible(kind:&Kind,binding:&str)->Option<bool>{Some(match binding{
    "music_volume"|"sfx_volume"|"text_speed"|"auto_speed"=>matches!(kind,Kind::Slider|Kind::Text),
    "typewriter"|"fullscreen"=>matches!(kind,Kind::CheckBox|Kind::Text),
    "language"=>matches!(kind,Kind::Select|Kind::Text),
    "save.thumbnail"|"gallery.thumbnail"=>*kind==Kind::Image,
    "save.slot"|"save.page"|"save.date"|"save.summary"|"dialogue.text"|"dialogue.name"|"confirmation.message"|"gallery.title"|"gallery.status"|"choice.text"|"history.text"|"history.name"=>*kind==Kind::Text,
    "dialogue.speaker"=>matches!(kind,Kind::Panel|Kind::Image),
    "save.save"|"save.load"|"save.delete"|"save.protect"=>*kind==Kind::Button,
    _=>return None,
})}
impl Element {
    /// Apply an outer list's presentation to an already laid-out component item.
    /// Item layout has already applied the component's own opacity.
    pub fn inherit_rendered_parent(&mut self,parent:&Element){
        self.enabled&=parent.enabled;self.locked|=parent.locked;
        let opacity=parent.appearance.opacity;self.appearance.opacity*=opacity;
        self.appearance.text_states.opacity(opacity);self.appearance.shadow.color[3]*=opacity;
        for rgba in [&mut self.normal,&mut self.hover,&mut self.pressed,&mut self.disabled,&mut self.foreground,&mut self.appearance.border_color]{rgba[3]*=opacity;}
        for rgba in [&mut self.appearance.focus,&mut self.appearance.selected].into_iter().flatten(){rgba[3]*=opacity;}
    }
}
impl Document {
    pub fn custom_save_pagination(&self,page:usize)->bool{
        self.layout_page(page,self.reference).iter().any(|e|matches!(e.action,Action::SavePage(_)))||self.pages.get(page).is_some_and(|p|p.graphs.iter().any(|g|g.nodes.iter().any(|n|matches!(n.op,Op::Action(Action::SavePage(_))))))
    }
    pub fn save_page_count(&self,page:usize)->Option<usize>{
        let lists:Vec<_>=self.layout_page(page,self.reference).into_iter().filter(|e|e.kind==Kind::SaveList).collect();
        (lists.len()==1).then(||lists[0].list.slots.div_ceil((lists[0].list.columns*lists[0].list.rows).max(1)))
    }
    pub fn add_quick_actions_page(&mut self)->usize{
        if let Some(index)=self.pages.iter().position(|p|p.role==Some(PageRole::QuickActions)){return index;}
        let mut n=0;while self.pages.iter().any(|p|p.id==format!("quick_actions_{n}")){n+=1;}
        let actions=[("Retour",Action::Rollback),("Historique",Action::History),("Sauv. rapide",Action::QuickSave),("Charger rapide",Action::QuickLoad),("Passer",Action::ToggleSkip),("Menu",Action::ToggleMenu)];
        let elements=actions.into_iter().enumerate().map(|(i,(label,action))|{let mut e=Element::new(format!("quick_{i}"),Kind::Button);e.name=label.into();e.text=label.into();e.action=action;e.rect=[400.0+i as f32*190.0,1038.0,182.0,38.0];e.font_size=22.0;e.normal=[0.03,0.035,0.05,0.85];e.appearance.radius=4.0;e.focus_order=i as i32;e}).collect();
        self.pages.push(Page{id:format!("quick_actions_{n}"),name:"Commandes rapides".into(),role:Some(PageRole::QuickActions),background:[0.0;4],elements,graphs:vec![]});self.pages.len()-1
    }
    pub fn add_choices_page(&mut self)->usize{
        if let Some(index)=self.pages.iter().position(|p|p.role==Some(PageRole::Choices)){return index;}
        let mut n=0;while self.pages.iter().any(|p|p.id==format!("choices_{n}")){n+=1;}
        let mut list=Element::new("responses".into(),Kind::ChoiceList);list.name="Réponses du scénario".into();list.rect=[460.0,160.0,1000.0,580.0];list.font_size=30.0;list.appearance.radius=8.0;list.layout_options.gap=14.0;
        self.pages.push(Page{id:format!("choices_{n}"),name:"Réponses aux choix".into(),role:Some(PageRole::Choices),background:[0.0;4],elements:vec![list],graphs:vec![]});self.pages.len()-1
    }
    pub fn add_dialogue_page(&mut self)->usize{
        if let Some(index)=self.pages.iter().position(|p|p.role==Some(PageRole::Dialogue)){return index;}
        let mut n=0;while self.pages.iter().any(|p|p.id==format!("dialogue_{n}")){n+=1;}
        let mut panel=Element::new("dialogue_panel".into(),Kind::Panel);panel.name="Boîte de dialogue".into();panel.rect=[60.0,820.0,1800.0,220.0];panel.normal=[0.025,0.03,0.045,0.94];panel.appearance.radius=12.0;
        let mut text=Element::new("dialogue_content".into(),Kind::Text);text.name="Texte du scénario".into();text.rect=[90.0,865.0,1740.0,140.0];text.font_size=30.0;text.binding=Some("dialogue.text".into());text.text="Votre dialogue apparaît ici, avec ses pauses et ses effets d’écriture.".into();
        let mut name=Element::new("dialogue_name".into(),Kind::Text);name.name="Nom du personnage".into();name.rect=[90.0,827.0,1000.0,38.0];name.font_size=28.0;name.binding=Some("dialogue.name".into());name.text="Personnage".into();
        self.pages.push(Page{id:format!("dialogue_{n}"),name:"Interface de dialogue".into(),role:Some(PageRole::Dialogue),background:[0.0;4],elements:vec![panel,name,text],graphs:vec![]});self.pages.len()-1
    }
    pub fn outline(&self,page:usize)->Vec<(String,usize,String)>{fn walk(es:&[Element],depth:usize,out:&mut Vec<(String,usize,String)>){for e in es{out.push((e.id.clone(),depth,e.name.clone()));walk(&e.children,depth+1,out)}}let mut out=vec![];if let Some(p)=self.pages.get(page){walk(&p.elements,0,&mut out)}out}
    pub fn remove_element(&mut self,page:usize,id:&str)->Option<Element>{fn take(es:&mut Vec<Element>,id:&str)->Option<Element>{if let Some(index)=es.iter().position(|e|e.id==id){return Some(es.remove(index))}for e in es{if let Some(found)=take(&mut e.children,id){return Some(found)}}None}take(&mut self.pages.get_mut(page)?.elements,id)}
    pub fn reparent(&mut self,page:usize,id:&str,parent:Option<&str>)->Result<(),String>{
        if Some(id)==parent{return Err(diagnostic!("Un élément ne peut pas se contenir lui-même", "An element cannot contain itself").into())}
        fn contains(e:&Element,id:&str)->bool{e.id==id||e.children.iter().any(|c|contains(c,id))}
        let e=self.find_element(page,id).ok_or(diagnostic!("Élément absent", "Missing element"))?;if parent.is_some_and(|p|contains(e,p)){return Err(diagnostic!("Imbrication cyclique interdite", "Cyclic nesting is not allowed").into())}
        if let Some(parent)=parent{let p=self.find_element(page,parent).ok_or(diagnostic!("Parent absent", "Missing parent"))?;if !matches!(p.kind,Kind::Panel|Kind::Overlay|Kind::Horizontal|Kind::Vertical|Kind::Grid|Kind::Scroll){return Err(diagnostic!("Le parent doit être un conteneur", "The parent must be a container").into())}}
        let e=self.remove_element(page,id).unwrap();if let Some(parent)=parent{self.find_element_mut(page,parent).unwrap().children.push(e)}else{self.pages[page].elements.push(e)}Ok(())
    }
    pub fn duplicate_element(&mut self,page:usize,id:&str)->Result<String,String>{let mut e=self.find_element(page,id).ok_or(diagnostic!("Élément absent", "Missing element"))?.clone();let ids:BTreeSet<_>=self.outline(page).into_iter().map(|e|e.0).collect();fn rename(e:&mut Element,ids:&BTreeSet<String>){let mut n=1;let old=e.id.clone();while ids.contains(&format!("{old}_copy{n}")){n+=1}e.id=format!("{old}_copy{n}");for child in &mut e.children{rename(child,ids)}}rename(&mut e,&ids);e.rect[0]+=24.0;e.rect[1]+=24.0;let id=e.id.clone();self.pages[page].elements.push(e);Ok(id)}
    pub fn reorder(&mut self,page:usize,id:&str,forward:bool){fn swap(es:&mut [Element],id:&str,forward:bool)->bool{if let Some(i)=es.iter().position(|e|e.id==id){if forward&&i+1<es.len(){es.swap(i,i+1)}else if !forward&&i>0{es.swap(i,i-1)}return true}for e in es{if swap(&mut e.children,id,forward){return true}}false}if let Some(p)=self.pages.get_mut(page){swap(&mut p.elements,id,forward);}}
    pub fn apply_theme(&mut self,preset:ThemePreset){
        let (bg,normal,accent,fg)=match preset{
            ThemePreset::Sobre=>([0.045,0.048,0.058,1.0],[0.13,0.14,0.17,1.0],[0.06,0.42,0.72,1.0],[0.95,0.95,0.97,1.0]),
            ThemePreset::Illustre=>([0.09,0.075,0.065,1.0],[0.18,0.14,0.11,0.86],[0.60,0.37,0.14,1.0],[1.0,0.95,0.84,1.0]),
            ThemePreset::ScienceFiction=>([0.012,0.035,0.06,1.0],[0.025,0.11,0.16,0.94],[0.0,0.58,0.69,1.0],[0.76,0.95,1.0,1.0]),
        };
        self.theme=preset;
        let button=StylePatch{normal:Some(normal),hover:Some(accent),pressed:Some(accent.map(|v|v*0.8)),foreground:Some(fg),focus:Some(accent),selected:Some(accent),radius:Some(if preset==ThemePreset::ScienceFiction{2.0}else{6.0}),..Default::default()};
        self.styles.insert("button".into(),button);
        self.styles.insert("text".into(),StylePatch{foreground:Some(fg),..Default::default()});
        self.styles.insert("panel".into(),StylePatch{normal:Some(normal),foreground:Some(fg),..Default::default()});
        fn link(es:&mut [Element]){for e in es{if e.style.is_none(){e.style=Some(match e.kind{Kind::Text=>"text",Kind::Button|Kind::CheckBox|Kind::Slider|Kind::Select=>"button",_=>"panel"}.into())}link(&mut e.children);}}
        for p in &mut self.pages{p.background=if matches!(p.role,Some(PageRole::Dialogue|PageRole::Choices|PageRole::QuickActions)){[0.0;4]}else{bg};link(&mut p.elements);}
    }
    pub fn resolved_element(&self,e:&Element)->Element{
        self.resolve_element(e,0)
    }
    fn resolve_element(&self,e:&Element,depth:usize)->Element{
        let mut resolved=e.clone();
        // Validation reports cycles. Keep inspection safe even for an invalid draft.
        if depth>=64{return resolved;}
        if let Some(template)=e.component.as_ref().and_then(|id|self.components.get(id)){
            let template=self.resolve_element(template,depth+1);
            if resolved.inherit_text{resolved.text=template.text.clone();}
            if resolved.inherit_action{resolved.action=template.action.clone();}
            if resolved.inherit_binding{resolved.binding=template.binding.clone();resolved.local_control=template.local_control.clone();}
            if resolved.visibility_binding.is_none(){resolved.visibility_binding=template.visibility_binding.clone();}
            resolved.asset=template.asset.clone();resolved.layout_options=template.layout_options.clone();resolved.list=template.list.clone();resolved.disabled=template.disabled;
            resolved.appearance=template.appearance.clone();resolved.kind=template.kind.clone();resolved.normal=template.normal;resolved.hover=template.hover;resolved.pressed=template.pressed;resolved.foreground=template.foreground;resolved.font=template.font.clone();resolved.font_size=template.font_size;
            if resolved.children.is_empty(){resolved.children=template.children.clone();fn prefix(es:&mut [Element],prefix:&str){for e in es{e.id=format!("{prefix}/{}",e.id);prefix_children(e);}}fn prefix_children(e:&mut Element){let id=e.id.clone();prefix(&mut e.children,&id);}prefix(&mut resolved.children,&e.id);}
            if resolved.style.is_none(){resolved.style=template.style.clone();}
            if let Some(style)=resolved.style.as_ref().and_then(|id|self.styles.get(id)){style.apply(&mut resolved)}
            template.overrides.apply(&mut resolved);
        }else if let Some(style)=e.style.as_ref().and_then(|id|self.styles.get(id)){style.apply(&mut resolved)}
        e.overrides.apply(&mut resolved);resolved
    }
    pub fn validate_design(&self)->Result<(),String>{
        for (index, page) in self.pages.iter().enumerate() {
            for graph in &page.graphs {
                if graph.event == Event::ValueChanged && !graph.target.as_deref()
                    .and_then(|id| self.find_element(index, id))
                    .is_some_and(|e| self.resolved_element(e).supports_value_changed()) {
                    return Err(diagnostic!("{} : Changement de valeur demande un contrôle relié à un réglage ou une variable d’interface", "{}: Value changed requires a control bound to a setting or a UI variable", graph.id));
                }
            }
        }
        for page in &self.pages{for graph in &page.graphs{if graph.event==Event::Click{if let Some(id)=&graph.target{let index=self.pages.iter().position(|p|p.id==page.id).unwrap();if self.find_element(index,id).is_some_and(|e|self.resolved_element(e).local_control.is_some()){return Err(diagnostic!("{} : utilisez l’événement Changement de valeur pour un contrôle local", "{}: use the Value changed event for a local control",graph.id));}}}}}
        for style in self.styles.values(){style.validate()?;}
        let mut roles=BTreeSet::new();for page in &self.pages{if let Some(role)=page.role{if !roles.insert(role){return Err(diagnostic!("Rôle de page dupliqué", "Duplicate page role").into())}}}
        fn check(doc:&Document,list:&[Element],stack:&mut Vec<String>,depth:usize)->Result<(),String>{
            if depth>64{return Err(diagnostic!("Hiérarchie trop profonde", "Hierarchy is too deep").into())}
            for e in list{
                if e.visibility_binding.as_deref().is_some_and(|key|!matches!(key,"always"|"save.empty"|"save.filled"|"save.locked"|"save.unlocked"|"gallery.locked"|"gallery.unlocked")){return Err(diagnostic!("{} : condition d’affichage inconnue", "{}: unknown visibility condition",e.name));}
                if e.kind==Kind::ChoiceList&&!e.layout_options.text_wrap{return Err(diagnostic!("Les réponses doivent conserver le retour à la ligne pour rester entièrement accessibles", "Choices must keep text wrapping enabled to remain fully accessible").into());}
                let resolved=doc.resolved_element(e);
                if let Some(control)=&resolved.local_control{control.validate(resolved.kind)?;if resolved.binding.is_some()||resolved.action!=Action::None{return Err(diagnostic!("{} : un contrôle local ne peut pas également déclencher une commande du jeu", "{}: a local control cannot also trigger a game command",e.name));}}
                if resolved.layout_options.auto_height&&(!matches!(resolved.kind,Kind::Text|Kind::Button)||resolved.anchors[1]!=resolved.anchors[3]){return Err(diagnostic!("{} : la hauteur automatique demande un texte ou bouton avec ancrage vertical fixe", "{}: automatic height requires text or a button with fixed vertical anchoring",e.name));}
                if resolved.layout_options.auto_height&&resolved.binding.as_deref()==Some("dialogue.text"){return Err(diagnostic!("Le texte de dialogue utilise sa zone défilante dédiée ; conservez sa hauteur fixe", "Dialogue text uses its dedicated scrolling area; keep its height fixed").into());}
                if let Action::OpenPage(id)=&resolved.action{if !doc.pages.iter().any(|p|p.id==*id){return Err(diagnostic!("Page cible absente : {id}", "Target page is missing: {id}"));}}
                if let Some(binding)=resolved.binding.as_deref().filter(|b|!b.is_empty()){
                    let kind=resolved.kind;
                    let compatible=binding_compatible(&kind,binding).ok_or_else(||diagnostic!("{} : donnée inconnue « {binding} »", "{}: unknown data binding “{binding}”",e.name))?;
                    if !compatible{return Err(diagnostic!("{} : le contrôle {:?} ne peut pas afficher « {binding} »", "{}: control {:?} cannot display “{binding}”",e.name,kind));}
                }
                e.appearance.validate()?;
                e.overrides.validate()?;
                if e.list.columns==0||e.list.columns>12||e.list.rows==0||e.list.rows>24||e.list.slots==0||e.list.slots>1000{return Err(diagnostic!("Dimensions de liste invalides", "Invalid list dimensions").into())}
                if e.list.template.as_ref().is_some_and(|id|!doc.components.contains_key(id)){return Err(diagnostic!("Modèle de carte absent", "Missing card template").into())}
                if let Some(template)=e.list.template.as_ref().filter(|_|matches!(e.kind,Kind::SaveList|Kind::Gallery)){
                    fn card_context(doc:&Document,e:&Element,prefix:&str,depth:usize)->Result<(),String>{
                        if depth>=64{return Err(diagnostic!("Composant de carte trop profond", "Card component nesting is too deep").into());}
                        let e=doc.resolved_element(e);
                        if let Some(key)=e.visibility_binding.as_deref().filter(|k|*k!="always"){if !key.starts_with(prefix){return Err(diagnostic!("{} : la condition {key} ne correspond pas aux données de cette carte ({prefix})", "{}: condition {key} does not match this card's data ({prefix})",e.name));}}
                        if let Some(key)=e.binding.as_deref().filter(|k|k.starts_with("save.")||k.starts_with("gallery.")){if !key.starts_with(prefix){return Err(diagnostic!("{} : la donnée {key} appartient à un autre type de carte", "{}: data binding {key} belongs to a different card type",e.name));}}
                        for child in &e.children{card_context(doc,child,prefix,depth+1)?;}Ok(())
                    }
                    card_context(doc,&doc.components[template],if e.kind==Kind::Gallery{"gallery."}else{"save."},0)?;
                }
                if let Some(template)=e.list.template.as_ref().filter(|_|matches!(e.kind,Kind::ChoiceList|Kind::History)){let binding=if e.kind==Kind::ChoiceList{"choice.text"}else{"history.text"};let rows=doc.layout_item(template,[1000.0,100.0],&BTreeMap::new());let texts=rows.iter().filter(|e|e.binding.as_deref()==Some(binding)).collect::<Vec<_>>();if texts.len()!=1||!texts[0].layout_options.text_wrap{return Err(diagnostic!("{} : le modèle doit contenir un unique texte à retour automatique lié à {binding}", "{}: the template must contain exactly one wrapping text element bound to {binding}",e.name));}}
                if e.style.as_ref().is_some_and(|id|!doc.styles.contains_key(id)){return Err(diagnostic!("Style absent : {}", "Missing style: {}",e.name))}
                let l=&e.layout_options;if l.padding.iter().chain(l.slice.iter()).chain(std::iter::once(&l.gap)).any(|v|!v.is_finite()||*v<0.0)||l.columns==0||l.columns>64{return Err(diagnostic!("Disposition invalide : {}", "Invalid layout: {}",e.name))}
                for id in e.component.iter().chain(e.list.template.iter()){if stack.contains(id){return Err(diagnostic!("Cycle de composants ou de modèles de liste", "Cycle in components or list templates").into())}let template=doc.components.get(id).ok_or_else(||diagnostic!("Composant absent : {id}", "Missing component: {id}"))?;stack.push(id.clone());check(doc,std::slice::from_ref(template),stack,depth+1)?;stack.pop();}
                check(doc,&e.children,stack,depth+1)?;
            }Ok(())
        }
        for (id,e) in &self.components{check(self,std::slice::from_ref(e),&mut vec![id.clone()],0)?;}
        let controls=self.local_controls()?;
        for page in &self.pages{for graph in &page.graphs{for node in &graph.nodes{if let Op::Set{variable,value}=&node.op{if let Some((kind,control))=controls.get(variable){if !control.accepts(*kind,value){return Err(diagnostic!("{} / nœud {} : valeur incompatible avec le contrôle {}", "{} / node {}: value incompatible with control {}",graph.id,node.id,variable));}}}}}}
        for (index,p) in self.pages.iter().enumerate(){
            check(self,&p.elements,&mut vec![],0)?;
            let elements=self.layout_page(index,self.reference);
            let texts=elements.iter().filter(|e|e.binding.as_deref()==Some("dialogue.text")).collect::<Vec<_>>();
            let choices=elements.iter().filter(|e|e.kind==Kind::ChoiceList).count();
            if p.role==Some(PageRole::Dialogue)&&(texts.len()!=1||texts[0].kind!=Kind::Text){return Err(diagnostic!("{} : ajoutez un unique Texte lié à dialogue.text", "{}: add exactly one Text element bound to dialogue.text",p.name));}
            if p.role==Some(PageRole::Dialogue)&&!texts[0].layout_options.text_wrap{return Err(diagnostic!("{} : activez le retour à la ligne du texte de dialogue pour garder les phrases longues consultables", "{}: enable dialogue text wrapping to keep long sentences accessible",p.name));}
            if p.role==Some(PageRole::Choices)&&choices!=1{return Err(diagnostic!("{} : une unique liste de réponses est requise", "{}: exactly one choice list is required",p.name));}
            if p.role.is_some()&&p.role!=Some(PageRole::Choices)&&choices>0{return Err(diagnostic!("La liste de réponses appartient à la page de rôle Choix", "The choice list belongs to the page with the Choices role").into());}
            if p.role==Some(PageRole::Confirm)&&(!elements.iter().any(|e|e.enabled&&e.kind==Kind::Button&&e.action==Action::Confirm)||!elements.iter().any(|e|e.enabled&&e.kind==Kind::Button&&e.action==Action::Back)){return Err(diagnostic!("Une confirmation doit proposer les boutons Confirmer et Retour / Annuler", "A confirmation must provide Confirm and Back / Cancel buttons").into());}
        }Ok(())
    }
    pub fn find_element<'a>(&'a self,page:usize,id:&str)->Option<&'a Element>{
        fn find<'a>(list:&'a [Element],id:&str)->Option<&'a Element>{for e in list{if e.id==id{return Some(e)}if let Some(found)=find(&e.children,id){return Some(found)}}None}
        find(&self.pages.get(page)?.elements,id)
    }
    pub fn history_data(&self,template:&str,name:&str,text:&str)->BTreeMap<String,String>{
        fn has_name(e:&Element)->bool{e.visible&&(e.binding.as_deref()==Some("history.name")||e.children.iter().any(has_name))}
        let separate=self.components.get(template).is_some_and(|e|has_name(&self.resolved_element(e)));
        let text=if separate||name.is_empty(){text.into()}else{format!("{name} : {text}")};
        [("history.text".into(),text),("history.name".into(),name.into())].into_iter().collect()
    }
    pub fn layout_item(&self,template:&str,size:[f32;2],data:&BTreeMap<String,String>)->Vec<Element>{
        self.layout_item_measured(template,size,data,&mut |_,_,_|None)
    }
    pub fn layout_item_measured<F:FnMut(&Element,f32,f32)->Option<f32>>(&self,template:&str,size:[f32;2],data:&BTreeMap<String,String>,measure:&mut F)->Vec<Element>{
        let Some(source)=self.components.get(template)else{return vec![]};let mut root=self.resolved_element(source);let reference=[root.rect[2],root.rect[3]];root.rect[0]=0.0;root.rect[1]=0.0;
        fn bind(doc:&Document,e:&mut Element,data:&BTreeMap<String,String>,depth:usize){if depth>=64{return;}*e=doc.resolved_element(e);e.component=None;if let Some(key)=e.visibility_binding.take(){e.visible&=key=="always"||data.get(&key).is_some_and(|v|v=="true");}if let Some(value)=e.binding.as_ref().and_then(|key|data.get(key)){if e.kind==Kind::Image{e.asset=if value.is_empty(){None}else{Some(value.clone())};e.overrides.image=Some(e.asset.clone().map(ResourceOverride::Path).unwrap_or(ResourceOverride::Clear));}else{e.text=value.clone();e.inherit_text=false}}for child in &mut e.children{bind(doc,child,data,depth+1)}}bind(self,&mut root,data,0);
        let mut doc=self.clone();doc.reference=reference;doc.pages=vec![Page{id:"item".into(),name:"item".into(),background:[0.0;4],elements:vec![root],graphs:vec![],role:None}];
        let original=doc.layout_page(0,size);let mut items=doc.layout_page_measured(0,size,measure);
        crate::card_layout::fit_card_fields(&original,&mut items);items
    }
    pub fn add_text_row(&mut self,history:bool)->String{
        let base=if history{"history_row"}else{"choice_row"};let mut id=base.to_string();let mut n=1;while self.components.contains_key(&id){id=format!("{base}_{n}");n+=1;}
        let mut root=Element::new("row".into(),Kind::Panel);root.rect=[0.0,0.0,1000.0,90.0];root.name=if history{"Ligne d’historique"}else{"Réponse au choix"}.into();root.style=Some("button".into());
        let mut text=Element::new("content".into(),Kind::Text);text.rect=[20.0,14.0,960.0,62.0];text.text=if history{"Personnage : une phrase de l’histoire…"}else{"La réponse proposée par le scénario"}.into();text.binding=Some(if history{"history.text"}else{"choice.text"}.into());text.font_size=30.0;text.style=Some("text".into());root.children.push(text);
        self.styles.entry("button".into()).or_default();self.styles.entry("text".into()).or_default();self.components.insert(id.clone(),root);id
    }
    pub fn add_save_card(&mut self)->String{
        // Legacy documents have no shared styles: do not restyle their existing pages.
        self.styles.entry("panel".into()).or_insert_with(||StylePatch{normal:Some([0.09,0.10,0.12,1.0]),..Default::default()});
        self.styles.entry("text".into()).or_insert_with(||StylePatch{foreground:Some([0.95,0.95,0.97,1.0]),..Default::default()});
        let mut n=1;while self.components.contains_key(&format!("save_card_{n}")){n+=1}let id=format!("save_card_{n}");let mut root=Element::new(id.clone(),Kind::Panel);root.name="Carte de sauvegarde".into();root.rect=[0.0,0.0,480.0,300.0];root.style=Some("panel".into());
        for (name,kind,rect,binding) in [("Miniature",Kind::Image,[12.0,12.0,456.0,210.0],"save.thumbnail"),("Date",Kind::Text,[12.0,230.0,456.0,28.0],"save.date"),("Résumé",Kind::Text,[12.0,264.0,456.0,28.0],"save.summary")]{let mut e=Element::new(format!("{id}_{name}"),kind);e.name=name.into();e.rect=rect;e.font_size=20.0;e.text=name.into();e.binding=Some(binding.into());e.style=Some("text".into());e.layout_options.auto_height=kind==Kind::Text;root.children.push(e);}
        self.components.insert(id.clone(),root);id
    }
    pub fn add_gallery_card(&mut self)->String{
        let old=self.add_save_card();let mut root=self.components.remove(&old).unwrap();let mut n=1;while self.components.contains_key(&format!("gallery_card_{n}")){n+=1;}let id=format!("gallery_card_{n}");root.id=id.clone();root.name="Carte de galerie".into();for e in &mut root.children{match e.binding.as_deref(){Some("save.thumbnail")=>{e.binding=Some("gallery.thumbnail".into());e.name="Illustration".into();},Some("save.date")=>{e.binding=Some("gallery.title".into());e.name="Titre".into();},_=>{e.binding=Some("gallery.status".into());e.name="Disponibilité".into();}}}self.components.insert(id.clone(),root);id
    }
    pub fn add_detailed_save_card(&mut self)->String{
        self.styles.entry("button".into()).or_insert_with(||StylePatch::from_element(&Element::new("button".into(),Kind::Button)));
        let id=self.add_save_card();let root=self.components.get_mut(&id).unwrap();root.name="Carte avec actions séparées".into();root.children[0].rect=[12.0,12.0,300.0,180.0];root.children[1].rect=[12.0,206.0,300.0,28.0];root.children[2].rect=[12.0,248.0,300.0,40.0];
        for(i,(text,binding))in [("Sauvegarder","save.save"),("Charger","save.load"),("Supprimer","save.delete"),("Protéger","save.protect")].into_iter().enumerate(){let mut e=Element::new(format!("{id}_action_{i}"),Kind::Button);e.name=text.into();e.text=text.into();e.binding=Some(binding.into());e.rect=[326.0,20.0+i as f32*64.0,142.0,44.0];e.font_size=18.0;e.overrides.font_size=Some(18.0);e.style=Some("button".into());root.children.push(e);}id
    }
    /// Isolated demonstration values: never reads actual player saves.
    pub fn preview_page(&self,page:usize,viewport:[f32;2])->Vec<Element>{
        self.preview_page_measured(page,viewport,&mut |_,_,_|None)
    }
    pub fn preview_page_measured<F:FnMut(&Element,f32,f32)->Option<f32>>(&self,page:usize,viewport:[f32;2],measure:&mut F)->Vec<Element>{
        let mut result=vec![];
        for mut e in self.layout_page_measured(page,viewport,measure){
            if e.binding.as_deref()==Some("save.page"){e.text=format!("{} : 1",e.text);}
            if matches!(e.kind,Kind::ChoiceList|Kind::History){if let Some(template)=&e.list.template{let Some(source)=self.components.get(template)else{result.push(e);continue;};let height=source.rect[3]*e.rect[2]/source.rect[2].max(1.0);for(i,text)in ["Une première phrase avec des accents : é, à, ç.","Une réponse plus longue qui s’adapte à l’espace disponible dans votre interface.","Une dernière ligne de démonstration."].iter().enumerate(){let data=[(if e.kind==Kind::ChoiceList{"choice.text"}else{"history.text"}.into(),(*text).into()),("history.name".into(),"Personnage".into())].into_iter().collect();for mut child in self.layout_item_measured(template,[e.rect[2],height],&data,measure){child.inherit_rendered_parent(&e);child.id=format!("{}/preview_{i}/{}",e.id,child.id);child.rect[0]+=e.rect[0];child.rect[1]+=e.rect[1]+i as f32*(height+e.layout_options.gap);result.push(child);}}continue;}}
            if e.kind==Kind::ChoiceList{for (i,text) in ["Partir","Ouvrir la porte","Lui révéler le secret — exemple d’une réponse plus longue"].iter().enumerate(){let mut row=e.clone();row.id=format!("{}/preview_{i}",e.id);row.kind=Kind::Button;row.text=(*text).into();row.rect[1]+=i as f32*(64.0+e.layout_options.gap);row.rect[3]=64.0;result.push(row);}continue;}
            result.push(e.clone());
            if !matches!(e.kind,Kind::SaveList|Kind::Gallery){continue}
            let Some(template)=&e.list.template else{continue};
            let mut cards=vec![];
            for index in 0..e.list.slots.min(e.list.columns*e.list.rows){
                let r=e.list.card_rect_with_navigation(index,[e.rect[2],e.rect[3]],e.layout_options.gap,e.kind!=Kind::SaveList||!self.custom_save_pagination(page));
                let data=if e.kind==Kind::Gallery{[("gallery.thumbnail".into(),String::new()),("gallery.title".into(),if index==0{"Illustration de démonstration".into()}else{"Illustration verrouillée".into()}),("gallery.status".into(),if index==0{"Disponible".into()}else{"À découvrir dans l’histoire".into()})].into_iter().collect()}else{[("save.slot".into(),(index+1).to_string()),("save.thumbnail".into(),String::new()),("save.date".into(),if index==0{"07/09/2026 14:30".into()}else{"Emplacement vide".into()}),("save.summary".into(),if index==0{"Une nouvelle histoire commence…".into()}else{format!("Emplacement {}",index+1)})].into_iter().collect()};
                let mut data:BTreeMap<String,String>=data;data.extend(card_state_bindings(e.kind==Kind::Gallery,index==0,e.kind==Kind::Gallery&&index!=0));
                cards.push((r,self.layout_item_measured(template,[r[2],r[3]],&data,measure)));
            }
            let mut bounds:Vec<_>=cards.iter().map(|(r,_)|*r).collect();
            crate::fit_card_rows(&mut bounds,&cards.iter().map(|(_,items)|items.first().map(|e|e.rect[3]).unwrap_or(0.0)).collect::<Vec<_>>(),e.layout_options.gap);
            for (index,((_,items),r)) in cards.into_iter().zip(bounds).enumerate(){for mut child in items{child.inherit_rendered_parent(&e);child.id=format!("{}/preview_{index}/{}",e.id,child.id);child.rect[0]+=e.rect[0]+r[0];child.rect[1]+=e.rect[1]+r[1];result.push(child);}}
        }
        result
    }
    pub fn find_element_mut<'a>(&'a mut self,page:usize,id:&str)->Option<&'a mut Element>{
        fn find<'a>(list:&'a mut [Element],id:&str)->Option<&'a mut Element>{for e in list{if e.id==id{return Some(e)}if let Some(found)=find(&mut e.children,id){return Some(found)}}None}
        find(&mut self.pages.get_mut(page)?.elements,id)
    }
    /// Materialize layout and styles into a flat, draw-order list in viewport pixels.
    pub fn layout_page(&self,page:usize,viewport:[f32;2])->Vec<Element>{
        self.layout_page_measured(page,viewport,&mut |_,_,_|None)
    }
    /// The host measures text with its real font renderer; shared layout places the
    /// expanded element and its following siblings without changing the document.
    pub fn layout_page_measured<F:FnMut(&Element,f32,f32)->Option<f32>>(&self,page:usize,viewport:[f32;2],measure:&mut F)->Vec<Element>{
        let Some(p)=self.pages.get(page)else{return vec![]};let scale=(viewport[0]/self.reference[0]).min(viewport[1]/self.reference[1]);let mut out=vec![];
        fn walk<F:FnMut(&Element,f32,f32)->Option<f32>>(doc:&Document,list:&[Element],parent:[f32;4],scale:f32,container:Option<&Element>,out:&mut Vec<Element>,depth:usize,measure:&mut F){
            if depth>64{return}let mut cursor=[0.0,0.0];let mut row_height=0.0;let mut index=0;
            for source in list{let mut e=doc.resolved_element(source);if !e.visible{continue}
                if let Some(parent)=container{e.enabled&=parent.enabled;e.locked|=parent.locked;e.appearance.opacity*=parent.appearance.opacity;}
                let mut r=[e.anchors[0]*parent[2]+e.rect[0]*scale,e.anchors[1]*parent[3]+e.rect[1]*scale,(e.anchors[2]-e.anchors[0])*parent[2]+e.rect[2]*scale,(e.anchors[3]-e.anchors[1])*parent[3]+e.rect[3]*scale];
                if e.layout_options.auto_height{if let Some(height)=measure(&e,r[2],scale).filter(|v|v.is_finite()&&*v>=0.0){r[3]=r[3].max(height);}}
                if let Some(c)=container{let gap=c.layout_options.gap*scale;match c.kind{Kind::Horizontal=>{r[0]=cursor[0];r[1]=0.0;cursor[0]+=r[2]+gap},Kind::Vertical=>{r[0]=0.0;r[1]=cursor[1];cursor[1]+=r[3]+gap},Kind::Grid=>{if index%c.layout_options.columns==0&&index>0{cursor[0]=0.0;cursor[1]+=row_height+gap;row_height=0.0;}r[0]=cursor[0];r[1]=cursor[1];cursor[0]+=r[2]+gap;row_height=row_height.max(r[3]);},_=>{}}}
                r[0]+=parent[0];r[1]+=parent[1];r[2]=r[2].max(0.0);r[3]=r[3].max(0.0);
                let children=e.children.clone();e.children.clear();e.rect=r;e.anchors=[0.0;4];e.font_size*=scale;e.appearance.scale(scale);let mut visual=e.clone();visual.layout_options.gap*=scale;visual.layout_options.padding=visual.layout_options.padding.map(|v|v*scale);let opacity=visual.appearance.opacity;visual.appearance.text_states.opacity(opacity);visual.appearance.shadow.color[3]*=opacity;for color in [&mut visual.normal,&mut visual.hover,&mut visual.pressed,&mut visual.disabled,&mut visual.foreground,&mut visual.appearance.border_color]{color[3]*=opacity;}if let Some(color)=&mut visual.appearance.focus{color[3]*=opacity;}if let Some(color)=&mut visual.appearance.selected{color[3]*=opacity;}out.push(visual);
                let padding=e.layout_options.padding.map(|v|v*scale);let content=[r[0]+padding[0],r[1]+padding[1],(r[2]-padding[0]-padding[2]).max(0.0),(r[3]-padding[1]-padding[3]).max(0.0)];
                walk(doc,&children,content,scale,Some(&e),out,depth+1,measure);index+=1;
            }
        }
        walk(self,&p.elements,[0.0,0.0,viewport[0],viewport[1]],scale,None,&mut out,0,measure);out
    }
    /// The same computed layout, preserving parentage for clipping and scrolling.
    /// Every rectangle is in viewport pixels relative to its immediate parent.
    pub fn layout_tree(&self,page:usize,viewport:[f32;2])->Vec<Element>{
        self.layout_tree_measured(page,viewport,&mut |_,_,_|None)
    }
    pub fn layout_tree_measured<F:FnMut(&Element,f32,f32)->Option<f32>>(&self,page:usize,viewport:[f32;2],measure:&mut F)->Vec<Element>{
        let Some(source)=self.pages.get(page)else{return vec![]};let flat:BTreeMap<_,_>=self.layout_page_measured(page,viewport,measure).into_iter().map(|e|(e.id.clone(),e)).collect();
        fn build(doc:&Document,source:&[Element],flat:&BTreeMap<String,Element>,origin:[f32;2],depth:usize)->Vec<Element>{
            if depth>64{return vec![];}source.iter().filter_map(|source|{let source=doc.resolved_element(source);let mut e=flat.get(&source.id)?.clone();let next=[e.rect[0],e.rect[1]];e.rect[0]-=origin[0];e.rect[1]-=origin[1];e.children=build(doc,&source.children,flat,next,depth+1);Some(e)}).collect()
        }
        build(self,&source.elements,&flat,[0.0;2],0)
    }
    pub fn layout_clips(&self,page:usize,viewport:[f32;2])->BTreeMap<String,[f32;4]>{
        fn walk(es:&[Element],origin:[f32;2],clip:[f32;4],out:&mut BTreeMap<String,[f32;4]>){for e in es{let pos=[origin[0]+e.rect[0],origin[1]+e.rect[1]];out.insert(e.id.clone(),clip);let mut next=clip;if e.kind==Kind::Scroll{let right=(clip[0]+clip[2]).min(pos[0]+e.rect[2]);let bottom=(clip[1]+clip[3]).min(pos[1]+e.rect[3]);next[0]=clip[0].max(pos[0]);next[1]=clip[1].max(pos[1]);next[2]=(right-next[0]).max(0.0);next[3]=(bottom-next[1]).max(0.0);}walk(&e.children,pos,next,out);}}
        let mut clips=BTreeMap::new();walk(&self.layout_tree(page,viewport),[0.0;2],[0.0,0.0,viewport[0],viewport[1]],&mut clips);clips
    }
}

#[cfg(test)]mod tests{
    #[test]fn card_data_requires_a_list_context_even_when_hidden(){
        let mut d=Document::defaults();let mut e=Element::new("slot".into(),Kind::Text);e.binding=Some("save.slot".into());e.visible=false;d.pages[0].elements=vec![e];
        assert!(d.validate().unwrap_err().contains("modèle de carte"));d.validate_authoring(&[d.pages[0].id.clone()].into_iter().collect()).unwrap();
    }
    #[test]fn slot_number_is_bound_and_previewed_without_changing_the_template(){
        let mut d=Document::defaults();let id=d.add_save_card();let mut number=Element::new("number".into(),Kind::Text);number.binding=Some("save.slot".into());d.components.get_mut(&id).unwrap().children.push(number);
        let mut list=Element::new("list".into(),Kind::SaveList);list.list=ListOptions{order:ListOrder::Rows,template:Some(id.clone()),columns:3,rows:2,slots:18};d.pages[0].elements=vec![list];d.validate().unwrap();let before=d.clone();
        let item=d.layout_item(&id,[480.0,300.0],&[("save.slot".into(),"13".into())].into_iter().collect());assert_eq!(item.iter().find(|e|e.id=="number").unwrap().text,"13");
        let preview=d.preview_page(0,d.reference);assert_eq!(preview.iter().find(|e|e.id=="list/preview_0/number").unwrap().text,"1");assert_eq!(preview.iter().find(|e|e.id=="list/preview_5/number").unwrap().text,"6");assert_eq!(d,before);
    }
    #[test]fn list_items_inherit_outer_opacity_and_disabled_state(){
        for kind in [Kind::SaveList,Kind::Gallery,Kind::ChoiceList,Kind::History]{
            let mut d=Document::defaults();let mut card=Element::new("card".into(),Kind::Panel);card.rect=[0.0,0.0,400.0,200.0];card.appearance.opacity=0.5;card.appearance.text_states.hover=Some([1.0;4]);d.components.insert("card".into(),card);
            let mut list=Element::new("list".into(),kind);list.rect=[0.0,0.0,1000.0,700.0];list.list.template=Some("card".into());list.appearance.opacity=0.5;list.enabled=false;d.pages[0].elements=vec![list];let before=d.clone();
            let preview=d.preview_page(0,d.reference);let item=preview.iter().find(|e|e.id=="list/preview_0/card").unwrap();
            assert_eq!(item.appearance.opacity,0.25);assert_eq!(item.foreground[3],0.25);assert_eq!(item.appearance.text_states.hover.unwrap()[3],0.25);assert!(!item.enabled);assert_eq!(d,before);
        }
    }
    #[test]fn hidden_grid_children_do_not_consume_cells(){
        let mut d=Document::defaults();let mut grid=Element::new("grid".into(),Kind::Grid);grid.rect=[0.0,0.0,600.0,400.0];grid.layout_options.columns=2;grid.layout_options.gap=10.0;
        for i in 0..4{let mut e=Element::new(format!("cell_{i}"),Kind::Text);e.rect=[0.0,0.0,100.0,40.0];e.visible=i!=1;grid.children.push(e);}
        d.pages[0].elements=vec![grid];let placed=d.layout_page(0,d.reference);let rect=|id:&str|placed.iter().find(|e|e.id==id).unwrap().rect;
        assert_eq!(rect("cell_0"),[0.0,0.0,100.0,40.0]);assert_eq!(rect("cell_2"),[110.0,0.0,100.0,40.0]);assert_eq!(rect("cell_3"),[0.0,50.0,100.0,40.0]);assert!(!placed.iter().any(|e|e.id=="cell_1"));
    }
    #[test]fn interactive_text_colors_inherit_and_respect_opacity(){
        let mut d=Document::defaults();let mut e=Element::new("caption".into(),Kind::Button);e.style=Some("caption".into());e.overrides.text_states.pressed=Some([1.0,0.0,0.0,1.0]);e.appearance.opacity=0.5;
        d.styles.insert("caption".into(),StylePatch{text_states:TextStateColors{hover:Some([1.0;4]),selected:Some([0.0,1.0,0.0,1.0]),..Default::default()},..Default::default()});d.pages[0].elements=vec![e];let before=d.clone();let e=d.layout_page(0,d.reference).remove(0);let s=&e.appearance.text_states;
        assert_eq!(s.resolve(e.foreground,true,false,true,false,false),[1.0,1.0,1.0,0.5]);assert_eq!(s.resolve(e.foreground,true,true,true,true,true),[1.0,0.0,0.0,0.5]);assert_eq!(s.resolve(e.foreground,false,true,true,true,true),e.foreground);assert_eq!(s.resolve(e.foreground,true,false,false,false,true),[0.0,1.0,0.0,0.5]);assert_eq!(d,before);
        d.pages[0].elements[0].overrides.text_states.hover=Some([2.0;4]);assert!(d.validate().is_err());
    }
    #[test]fn component_content_inherits_without_losing_local_exceptions(){
        let mut d=Document::defaults();let mut shared=Element::new("shared".into(),Kind::Text);shared.text="Page".into();shared.binding=Some("save.page".into());shared.action=Action::Back;d.components.insert("shared".into(),shared.clone());
        let mut instance=shared.clone();instance.id="instance".into();instance.component=Some("shared".into());instance.inherit_text=true;instance.inherit_action=true;instance.inherit_binding=true;
        d.pages[0].elements.push(instance.clone());let shared=d.components.get_mut("shared").unwrap();shared.text="Volume".into();shared.action=Action::Settings;shared.binding=Some("music_volume".into());
        let e=d.resolved_element(&instance);assert_eq!(e.text,"Volume");assert_eq!(e.action,Action::Settings);assert_eq!(e.binding.as_deref(),Some("music_volume"));
        instance.inherit_text=false;instance.text="Mon texte".into();instance.inherit_action=false;instance.action=Action::None;instance.inherit_binding=false;instance.binding=None;
        let e=d.resolved_element(&instance);assert_eq!(e.text,"Mon texte");assert_eq!(e.action,Action::None);assert_eq!(e.binding,None);
        instance.inherit_text=true;assert_eq!(d.resolved_element(&instance).text,"Volume");
        let before=d.clone();let mut session=Session::default();session.apply_presentation(&d.pages[0].id,&Effect::Text("instance".into(),"Temporaire".into()));let presented=session.present(&d);assert_eq!(presented.resolved_element(presented.find_element(0,"instance").unwrap()).text,"Temporaire");assert_eq!(d,before);
        let roundtrip:Document=serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();assert_eq!(roundtrip,d);
        let mut legacy=serde_json::to_value(&instance).unwrap();for field in ["inherit_text","inherit_action","inherit_binding"]{legacy.as_object_mut().unwrap().remove(field);}let legacy:Element=serde_json::from_value(legacy).unwrap();assert_eq!(d.resolved_element(&legacy).text,"Mon texte");assert!(!legacy.inherit_text);
    }
    #[test]fn pagination_component_workspace_is_not_an_exportable_page(){
        let mut d=Document::defaults();let mut e=Element::new("next".into(),Kind::Button);e.action=Action::SavePage(SavePage::Next);d.pages[0].elements=vec![e];
        assert!(d.validate().unwrap_err().contains("pagination"));let workspaces=[d.pages[0].id.clone()].into_iter().collect();d.validate_authoring(&workspaces).unwrap();
        d.pages[0].elements[0].normal=[2.0;4];assert!(d.validate_authoring(&workspaces).is_err());
    }
    #[test]fn inherited_actions_are_validated_against_real_destinations(){
        let mut d=Document::defaults();let mut source=Element::new("shared".into(),Kind::Button);source.action=Action::StartScene("absent".into());d.components.insert("shared".into(),source);let mut instance=Element::new("instance".into(),Kind::Button);instance.component=Some("shared".into());instance.inherit_action=true;d.pages[0].elements.push(instance);
        assert!(d.validate_files(std::path::Path::new("/tmp"),&BTreeSet::new()).unwrap_err().contains("Label de menu absent"));
        d.components.get_mut("shared").unwrap().action=Action::OpenPage("absent".into());assert!(d.validate().unwrap_err().contains("Page cible absente"));
    }
    #[test]fn card_visibility_binds_nested_components_without_changing_the_design(){
        let mut d=Document::defaults();let card=d.add_save_card();let mut marker=Element::new("marker".into(),Kind::Text);marker.text="EMPLACEMENT VIDE".into();marker.visibility_binding=Some("save.empty".into());d.components.insert("marker".into(),marker);
        let mut instance=Element::new("empty_marker".into(),Kind::Text);instance.component=Some("marker".into());d.components.get_mut(&card).unwrap().children.push(instance);d.validate().unwrap();let before=d.clone();
        let empty=d.layout_item(&card,[480.0,300.0],&card_state_bindings(false,false,false));assert!(empty.iter().any(|e|e.id=="empty_marker"));
        let filled=d.layout_item(&card,[480.0,300.0],&card_state_bindings(false,true,false));assert!(!filled.iter().any(|e|e.id=="empty_marker"));assert_eq!(d,before);
        d.components.get_mut(&card).unwrap().children.last_mut().unwrap().visibility_binding=Some("always".into());
        assert!(d.layout_item(&card,[480.0,300.0],&card_state_bindings(false,true,false)).iter().any(|e|e.id=="empty_marker"));
        d.components.get_mut("marker").unwrap().visibility_binding=Some("scenario.secret".into());assert!(d.validate().is_err());
    }
    #[test]fn nine_slice_source_coordinates_do_not_change_with_viewport(){
        let mut d=Document::defaults();let mut e=Element::new("frame".into(),Kind::Image);e.layout_options.image_fit=ImageFit::NineSlice;e.layout_options.slice=[100.0,40.0,100.0,40.0];d.pages[0].elements=vec![e];
        for size in [[960.0,540.0],[1920.0,1080.0],[2560.0,1080.0]]{assert_eq!(d.layout_page(0,size)[0].layout_options.slice,[100.0,40.0,100.0,40.0]);}
    }
    #[test]fn scrollbar_style_is_inherited_with_independent_local_states(){
        let mut d=Document::defaults();let mut e=Element::new("scroll".into(),Kind::Scroll);e.style=Some("scroll_style".into());
        d.styles.insert("scroll_style".into(),StylePatch{scrollbar:ScrollbarPatch{track:Some([0.1;4]),thumb:Some([0.8;4]),width:Some(24.0),..Default::default()},..Default::default()});
        e.overrides.scrollbar.thumb=Some([1.0,0.0,0.0,1.0]);let s=d.resolved_element(&e).appearance.scrollbar;assert_eq!(s.track,[0.1;4]);assert_eq!(s.thumb,[1.0,0.0,0.0,1.0]);assert_eq!(s.width,24.0);
        e.overrides.scrollbar.thumb=None;assert_eq!(d.resolved_element(&e).appearance.scrollbar.thumb,[0.8;4]);
        e.overrides.scrollbar.width=Some(f32::NAN);assert!(e.overrides.validate().is_err());
    }
    #[test]fn shadow_inherits_scales_and_is_disabled_by_default(){
        let mut d=Document::defaults();let mut e=Element::new("panel".into(),Kind::Panel);e.rect=[40.0,50.0,200.0,100.0];
        assert!(shadow_layers(e.rect,0.0,&e.appearance.shadow).is_empty());
        e.style=Some("shadow".into());d.styles.insert("shadow".into(),StylePatch{shadow:Some(Shadow{offset:[12.0,16.0],blur:20.0,color:[0.0,0.0,0.0,0.8]}),..Default::default()});e.appearance.opacity=0.5;d.pages[0].elements=vec![e];
        let flat=d.layout_page(0,[960.0,540.0]);let s=&flat[0].appearance.shadow;assert_eq!(s.offset,[6.0,8.0]);assert_eq!(s.blur,10.0);assert!((s.color[3]-0.4).abs()<0.0001);assert_eq!(shadow_layers(flat[0].rect,0.0,s).len(),8);
        let before=d.clone();d.pages[0].elements[0].overrides.shadow=Some(Shadow::default());assert!(shadow_layers(flat[0].rect,0.0,&d.resolved_element(&d.pages[0].elements[0]).appearance.shadow).is_empty());assert_ne!(d,before);
        assert!(Shadow{blur:f32::NAN,..Default::default()}.validate().is_err());d.validate().unwrap();
    }
    #[test]fn history_name_is_not_duplicated_in_a_separate_field(){
        let mut d=Document::defaults();let id=d.add_text_row(true);
        assert_eq!(d.history_data(&id,"Mara","Bonjour")["history.text"],"Mara : Bonjour");
        let mut name=Element::new("speaker".into(),Kind::Text);name.binding=Some("history.name".into());d.components.get_mut(&id).unwrap().children.push(name);
        assert_eq!(d.history_data(&id,"Mara","Bonjour")["history.text"],"Bonjour");
        assert_eq!(d.history_data(&id,"","Narration")["history.text"],"Narration");
        assert_eq!(d.history_data(&id,"","Narration")["history.name"],"");
    }
    use super::*;
    #[test]fn dynamic_text_templates_validate_bind_and_preserve_scenario(){let mut d=Document::defaults();let index=d.add_choices_page();let id=d.add_text_row(false);d.pages[index].elements[0].list.template=Some(id.clone());d.validate().unwrap();let old=d.clone();let text="Un texte long, avec des accents et une destination inchangée.";let data=[("choice.text".into(),text.into())].into_iter().collect();assert!(d.layout_item(&id,[900.0,90.0],&data).iter().any(|e|e.text==text));assert_eq!(d,old);assert!(d.preview_page(index,d.reference).iter().any(|e|e.text.contains("première")));d.components.get_mut(&id).unwrap().children.clear();assert!(d.validate().unwrap_err().contains("choice.text"));}
    #[test]fn opacity_is_inherited_once_and_preserves_source(){
        let mut d=Document::defaults();let mut parent=Element::new("parent".into(),Kind::Panel);parent.appearance.opacity=0.5;
        let mut child=Element::new("child".into(),Kind::Image);child.overrides.opacity=Some(0.5);child.foreground=[1.0;4];parent.children.push(child);d.pages[0].elements=vec![parent];let original=d.clone();
        let flat=d.layout_page(0,d.reference);assert_eq!(flat[1].appearance.opacity,0.25);assert_eq!(flat[1].foreground[3],0.25);assert_eq!(d.layout_tree(0,d.reference)[0].children[0].foreground[3],0.25);assert_eq!(d,original);
        assert_eq!(Document::from_json(&serde_json::to_string(&d).unwrap()).unwrap(),d);assert_eq!(serde_json::from_str::<Appearance>("{}").unwrap().opacity,1.0);
        d.pages[0].elements[0].overrides.opacity=Some(1.5);assert!(d.validate().is_err());
    }
    #[test]fn disabled_parent_and_scroll_clips_reach_descendants(){let mut d=Document::defaults();let mut root=Element::new("root".into(),Kind::Scroll);root.rect=[10.0,20.0,200.0,100.0];root.enabled=false;root.locked=true;let mut child=Element::new("child".into(),Kind::Button);child.rect=[0.0,90.0,180.0,80.0];root.children.push(child);d.pages[0].elements=vec![root];let flat=d.layout_page(0,d.reference);assert!(!flat[1].enabled);assert!(flat[1].locked);assert_eq!(d.layout_clips(0,d.reference)["child"],[10.0,20.0,200.0,100.0]);assert!(d.pages[0].elements[0].children[0].enabled);}
    #[test]fn nested_render_tree_preserves_shared_world_coordinates(){let mut d=Document::defaults();let mut scroll=Element::new("scroll".into(),Kind::Scroll);scroll.rect=[100.0,100.0,600.0,300.0];scroll.layout_options.padding=[20.0;4];let mut column=Element::new("column".into(),Kind::Vertical);column.rect=[0.0,0.0,400.0,600.0];column.children=(0..6).map(|i|Element::new(format!("item_{i}"),Kind::Button)).collect();scroll.children.push(column);d.pages[0].elements=vec![scroll];for size in [[1280.0,720.0],[1920.0,1080.0],[2560.0,1080.0]]{let flat=d.layout_page(0,size);let tree=d.layout_tree(0,size);assert_eq!(tree[0].children.len(),1);assert_eq!(tree[0].children[0].children.len(),6);for child in &tree[0].children[0].children{let world=[tree[0].rect[0]+tree[0].children[0].rect[0]+child.rect[0],tree[0].rect[1]+tree[0].children[0].rect[1]+child.rect[1]];let original=flat.iter().find(|e|e.id==child.id).unwrap();assert!((world[0]-original.rect[0]).abs()<0.01&&(world[1]-original.rect[1]).abs()<0.01);assert_eq!(child.font_size,original.font_size);}}}
    #[test]fn transient_image_and_nested_component_are_resolved(){
        let mut d=Document::defaults();let mut base=Element::new("base".into(),Kind::Image);base.asset=Some("base.png".into());d.components.insert("base".into(),base);
        let mut variant=Element::new("variant".into(),Kind::Image);variant.component=Some("base".into());variant.overrides.image=Some(ResourceOverride::Path("variant.png".into()));d.components.insert("variant".into(),variant);
        let mut instance=Element::new("instance".into(),Kind::Image);instance.component=Some("variant".into());d.pages[0].elements.push(instance.clone());
        assert_eq!(d.resolved_element(&instance).asset.as_deref(),Some("variant.png"));let before=d.clone();
        let mut session=Session::default();session.apply_presentation(&d.pages[0].id,&Effect::Image("instance".into(),"temporary.png".into()));let shown=session.present(&d);assert_eq!(shown.resolved_element(shown.pages[0].elements.last().unwrap()).asset.as_deref(),Some("temporary.png"));assert_eq!(before,d);
    }
    #[test]fn rejects_unknown_or_incompatible_bindings(){let mut d=Document::defaults();d.pages[0].elements[0].binding=Some("not.a.binding".into());assert!(d.validate().unwrap_err().contains("inconnue"));d.pages[0].elements[0].binding=Some("save.thumbnail".into());assert!(d.validate().unwrap_err().contains("ne peut pas"));}
    #[test]fn component_image_changes_preserve_explicit_exceptions(){
        let mut d=Document::defaults();let mut template=Element::new("shared".into(),Kind::Image);template.asset=Some("one.png".into());d.components.insert("photo".into(),template);
        let mut instance=Element::new("instance".into(),Kind::Image);instance.component=Some("photo".into());
        assert_eq!(d.resolved_element(&instance).asset.as_deref(),Some("one.png"));
        instance.overrides.image=Some(ResourceOverride::Path("local.png".into()));d.components.get_mut("photo").unwrap().asset=Some("two.png".into());
        assert_eq!(d.resolved_element(&instance).asset.as_deref(),Some("local.png"));
        instance.overrides.image=Some(ResourceOverride::Clear);assert_eq!(d.resolved_element(&instance).asset,None);
        instance.overrides.image=None;assert_eq!(d.resolved_element(&instance).asset.as_deref(),Some("two.png"));
        d.pages[0].elements.push(instance);let json=d.to_json().unwrap();assert_eq!(Document::from_json(&json).unwrap(),d);
    }
    #[test]fn dialogue_model_is_unique_and_editable(){let mut d=Document::defaults();let index=d.add_dialogue_page();assert_eq!(d.add_dialogue_page(),index);d.validate().unwrap();let elements=d.layout_page(index,[1280.0,720.0]);assert_eq!(elements.iter().filter(|e|e.binding.as_deref()==Some("dialogue.text")).count(),1);assert!(elements.iter().all(|e|e.rect[0]>=0.0&&e.rect[1]>=0.0&&e.rect[0]+e.rect[2]<=1280.1&&e.rect[1]+e.rect[3]<=720.1));}
    #[test]fn border_styles_resolve_and_scale_without_mutating_source(){
        let mut d=Document::defaults();let e=&mut d.pages[0].elements[1];e.overrides.radius=Some(18.0);e.overrides.border_width=Some(4.0);e.overrides.border_color=Some([0.0,1.0,1.0,1.0]);let id=e.id.clone();let before=d.clone();
        let resolved=d.layout_page(0,[960.0,540.0]).into_iter().find(|e|e.id==id).unwrap();assert_eq!(resolved.appearance.radius,9.0);assert_eq!(resolved.appearance.border_width,2.0);assert_eq!(resolved.appearance.border_color,[0.0,1.0,1.0,1.0]);assert_eq!(d,before);
        d.pages[0].elements[1].overrides.border_width=Some(-1.0);assert!(d.validate().is_err());
    }
    #[test]fn rejects_recursive_list_template(){let mut d=Document::defaults();let mut list=Element::new("list".into(),Kind::SaveList);list.list.template=Some("card".into());d.components.insert("card".into(),list);assert!(d.validate().unwrap_err().contains("Cycle"));}
    #[test]fn save_card_in_legacy_document_is_valid(){let mut d=Document::defaults();d.styles.clear();for p in &mut d.pages{for e in &mut p.elements{e.style=None;}}let pages=d.pages.clone();d.add_save_card();d.validate().unwrap();assert_eq!(d.pages,pages);}
    #[test]fn incompatible_card_data_is_rejected_when_linking_the_template(){
        let mut d=Document::defaults();let card=d.add_save_card();let page=d.pages.iter_mut().find(|p|p.id=="save").unwrap();page.elements.iter_mut().find(|e|e.kind==Kind::SaveList).unwrap().list.template=Some(card.clone());
        d.validate().unwrap();d.components.get_mut(&card).unwrap().children[0].visibility_binding=Some("gallery.locked".into());assert!(d.validate().unwrap_err().contains("condition gallery.locked"));
        d.components.get_mut(&card).unwrap().children[0].visibility_binding=None;d.components.get_mut(&card).unwrap().children[0].binding=Some("gallery.thumbnail".into());assert!(d.validate().unwrap_err().contains("autre type de carte"));
    }
    #[test]fn pagination_is_typed_bounded_and_uses_the_authored_space(){
        let mut d=Document::from_template(ThemePreset::Sobre);let page=d.pages.iter().position(|p|p.id=="save").unwrap();let count=d.save_page_count(page).unwrap();assert_eq!(count,10);
        d.pages[page].elements[1].action=Action::SavePage(SavePage::Next);assert!(d.custom_save_pagination(page));d.validate().unwrap();
        let list=ListOptions{order:ListOrder::Rows,columns:3,rows:2,slots:60,template:None};assert_eq!(list.card_rect_with_navigation(5,[900.0,600.0],0.0,false),[600.0,300.0,300.0,300.0]);
        assert_eq!(SavePage::Next.resolve(9,count),9);assert_eq!(SavePage::Previous.resolve(0,count),0);assert_eq!(SavePage::Number(3).resolve(0,count),2);
        d.pages[page].elements[1].action=Action::SavePage(SavePage::Number(11));assert!(d.validate().unwrap_err().contains("hors limites"));
        d.pages[page].elements[1].action=Action::SavePage(SavePage::Next);let extra=d.pages[page].elements.iter().find(|e|e.kind==Kind::SaveList).unwrap().clone();let mut extra=extra;extra.id="second_list".into();d.pages[page].elements.push(extra);assert!(d.validate().unwrap_err().contains("unique liste"));
    }
    #[test]fn nine_slice_preserves_corners(){let quads=image_quads([100.0,100.0],[400.0,200.0],ImageFit::NineSlice,[10.0,12.0,10.0,12.0]);assert_eq!(quads.len(),9);assert_eq!(quads[0].0,[0.0,0.0,10.0,12.0]);assert_eq!(quads[4].0,[10.0,12.0,380.0,176.0]);assert!(image_quads([100.0;2],[400.0;2],ImageFit::NineSlice,[-1.0;4]).is_empty());}
    #[test]fn image_fit_does_not_distort(){assert_eq!(image_quads([200.0,100.0],[100.0;2],ImageFit::Contain,[0.0;4])[0].0,[0.0,25.0,100.0,50.0]);assert_eq!(image_quads([200.0,100.0],[100.0;2],ImageFit::Cover,[0.0;4])[0].1,[0.25,0.0,0.5,1.0]);}
    #[test]fn card_bindings_do_not_mutate_template(){let mut d=Document::defaults();let id=d.add_save_card();let before=d.clone();let data=[("save.date".into(),"07/09/2026".into()),("save.thumbnail".into(),"thumb.png".into())].into_iter().collect();let card=d.layout_item(&id,[480.0,300.0],&data);assert!(card.iter().any(|e|e.text=="07/09/2026"));assert!(card.iter().any(|e|e.asset.as_deref()==Some("thumb.png")));assert_eq!(d,before);}
    #[test]fn six_cards_fit_without_pagination_space(){let list=ListOptions{order:ListOrder::Rows,columns:3,rows:2,slots:6,template:None};assert_eq!(list.card_rect(5,[900.0,600.0],0.0),[600.0,300.0,300.0,300.0]);}
    #[test]fn reparent_cycle_leaves_document_intact(){let mut d=Document::defaults();let mut root=Element::new("root".into(),Kind::Panel);root.children.push(Element::new("child".into(),Kind::Panel));d.pages[0].elements=vec![root];let old=d.clone();assert!(d.reparent(0,"root",Some("child")).is_err());assert_eq!(d,old);d.reparent(0,"child",None).unwrap();assert_eq!(d.pages[0].elements.len(),2);}
    #[test]fn legacy_migration_preserves_values(){let mut d=Document::defaults();d.styles.clear();for p in &mut d.pages{for e in &mut p.elements{e.style=None;}}let mut json=serde_json::to_value(&d).unwrap();json["version"]=1.into();let migrated=Document::from_json(&json.to_string()).unwrap();assert_eq!(migrated.version,2);assert_eq!(migrated.pages,d.pages);}
    #[test]fn theme_preserves_interaction_geometry_and_overrides(){let mut d=Document::defaults();let old=d.pages[0].elements[1].clone();d.pages[0].elements[1].overrides.foreground=Some([1.0,0.0,0.0,1.0]);d.apply_theme(ThemePreset::ScienceFiction);let e=d.resolved_element(&d.pages[0].elements[1]);assert_eq!(e.rect,old.rect);assert_eq!(e.action,old.action);assert_eq!(e.foreground,[1.0,0.0,0.0,1.0]);assert_ne!(e.normal,old.normal);}
    #[test]fn rejects_component_cycle(){let mut d=Document::defaults();let mut e=Element::new("cycle".into(),Kind::Button);e.component=Some("cycle".into());d.components.insert("cycle".into(),e);assert!(d.validate().is_err());}
    #[test]fn nested_layout_follows_parent(){let mut d=Document::defaults();let mut p=Element::new("p".into(),Kind::Vertical);p.rect=[100.0,50.0,400.0,400.0];p.children=vec![Element::new("a".into(),Kind::Button),Element::new("b".into(),Kind::Button)];d.pages[0].elements=vec![p];let es=d.layout_page(0,[1920.0,1080.0]);assert_eq!(es[1].rect[0],100.0);assert_eq!(es[2].rect[1],126.0);}
}
