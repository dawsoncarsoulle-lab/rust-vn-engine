//! Engine-independent menu documents, layout and bounded interaction graphs.
#[macro_use]
mod diagnostics;
pub use diagnostics::set_diagnostic_english;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

mod design;
mod card_layout;
pub use card_layout::fit_card_rows;
mod style_names;
mod controls;
pub use controls::LocalControl;
mod animation;
pub use animation::*;
pub use design::*;
mod editing;
mod templates;
mod theme_import;
pub use theme_import::ThemeImport;
pub use editing::*;
mod package;
pub use package::*;
pub const VERSION: u32 = 2;
/// Optional [paths].menus entry, relative to the project (never a global path).
pub fn document_path(root: &Path) -> Result<std::path::PathBuf, String> {
    let manifest = root.join("rvn.toml");
    let path = if manifest.is_file() {
        let source = std::fs::read_to_string(manifest).map_err(|e| e.to_string())?;
        let value: toml::Value = toml::from_str(&source).map_err(|e| e.to_string())?;
        value
            .get("paths")
            .and_then(|p| p.get("menus"))
            .and_then(|p| p.as_str())
            .unwrap_or("menus.rvnui")
            .to_string()
    } else {
        "menus.rvnui".into()
    };
    let path = Path::new(&path);
    if path.as_os_str().is_empty()
        || path.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(diagnostic!("Le fichier de menus doit rester dans le dossier du projet", "The menu file must remain inside the project folder").into());
    }
    Ok(root.join(path))
}

/// Refuse to overwrite external changes; publish a complete document atomically.
pub fn save_document(
    path: &Path,
    doc: &Document,
    expected: Option<&str>,
) -> Result<String, String> {
    use std::io::Write;
    let current = match std::fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if current.as_deref() != expected {
        return Err(diagnostic!("Les menus ont été modifiés sur disque. Rouvrez le projet avant d’enregistrer pour ne pas les écraser.", "Menus have changed on disk. Reopen the project before saving to avoid overwriting them.").into());
    }
    let json = doc.to_json()?;
    let parent = path.parent().ok_or(diagnostic!("Dossier de menus absent", "Missing menu folder"))?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if let Some(source)=current.as_ref().filter(|s|serde_json::from_str::<serde_json::Value>(s).ok().and_then(|v|v.get("version").and_then(|v|v.as_u64()))==Some(1)){
        let mut backup=path.with_extension("rvnui.v1.bak");let mut index=1;
        while backup.exists(){if std::fs::read_to_string(&backup).ok().as_ref()==Some(source){break}backup=path.with_extension(format!("rvnui.v1.{index}.bak"));index+=1;}
        if !backup.exists(){let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&backup).map_err(|e|e.to_string())?;file.write_all(source.as_bytes()).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;}
    }
    let temp = path.with_extension(format!("rvnui.{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(json.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result?;
    Ok(json)
}
pub type Color = [f32; 4];
mod image_states;
pub use image_states::StateImages;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub version: u32,
    pub reference: [f32; 2],
    pub pages: Vec<Page>,
    #[serde(default)] pub styles:BTreeMap<String,StylePatch>,
    #[serde(default)] pub components:BTreeMap<String,Element>,
    #[serde(default)] pub theme:ThemePreset,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub background: Color,
    pub elements: Vec<Element>,
    #[serde(default)]
    pub graphs: Vec<Graph>,
    #[serde(default)] pub role:Option<PageRole>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Panel,
    Text,
    Image,
    Button,
    CheckBox,
    Slider,
    Select,
    Horizontal,
    Vertical,
    Scroll,
    SaveList,
    ChoiceList,
    Gallery,
    History,
    Grid,
    Overlay,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Element {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub rect: [f32; 4],
    /// min-x,min-y,max-x,max-y; equal anchors preserve size, distinct anchors stretch.
    pub anchors: [f32; 4],
    pub text: String,
    pub asset: Option<String>,
    pub font: Option<String>,
    pub font_size: f32,
    pub normal: Color,
    pub hover: Color,
    pub pressed: Color,
    pub disabled: Color,
    pub foreground: Color,
    pub visible: bool,
    pub enabled: bool,
    pub locked: bool,
    pub focus_order: i32,
    pub action: Action,
    #[serde(default)]
    pub binding: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub local_control:Option<LocalControl>,
    #[serde(default)]
    pub visibility_binding: Option<String>,
    #[serde(default)]
    pub children: Vec<Element>,
    #[serde(default)] pub style:Option<String>,
    #[serde(default)] pub overrides:StylePatch,
    #[serde(default)] pub component:Option<String>,
    /// Explicit opt-in preserves the content of existing v2 instances.
    #[serde(default)] pub inherit_text:bool,
    #[serde(default)] pub inherit_action:bool,
    #[serde(default)] pub inherit_binding:bool,
    #[serde(default)] pub layout_options:LayoutOptions,
    #[serde(default)] pub list:ListOptions,
    #[serde(default)] pub appearance:Appearance,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Action {
    None,
    Continue,
    NewGame,
    Resume,
    Save,
    Load,
    Settings,
    Gallery,
    History,
    Quit,
    Back,
    Confirm,
    QuickSave,
    QuickLoad,
    Rollback,
    ToggleMenu,
    ToggleSkip,
    OpenPage(String),
    StartScene(String),
    SavePage(SavePage),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SavePage {Previous,Next,First,Last,Number(usize)}
impl SavePage {
    pub fn resolve(&self,current:usize,pages:usize)->usize{let last=pages.saturating_sub(1);match self{Self::Previous=>current.saturating_sub(1),Self::Next=>current.saturating_add(1),Self::First=>0,Self::Last=>last,Self::Number(n)=>n.saturating_sub(1)}.min(last)}
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Event {
    Click,
    Hover,
    HoverLeave,
    Focus,
    ValueChanged,
    Open,
    Close,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Graph {
    pub id: String,
    pub target: Option<String>,
    pub event: Event,
    pub entry: u32,
    pub nodes: Vec<Node>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: u32,
    pub position: [f32; 2],
    pub op: Op,
    pub next: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Op {
    Action(Action),
    Visible {
        element: String,
        value: bool,
    },
    Enabled {
        element: String,
        value: bool,
    },
    Text {
        element: String,
        value: String,
    },
    Image {
        element: String,
        path: String,
    },
    Sound {path:String},
    Animate {element:String,clip:AnimationClip},
    Set {
        variable: String,
        value: serde_json::Value,
    },
    Branch {
        variable: String,
        equals: serde_json::Value,
        otherwise: Option<u32>,
    },
}
#[derive(Clone,Default)]
pub struct Session {
    pub variables: BTreeMap<String, serde_json::Value>,
    pub presentation: BTreeMap<(String,String),ElementState>,
    pub last_error: Option<RuntimeDiagnostic>,
    cursor: Option<u32>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct RuntimeDiagnostic {pub page:String,pub graph:String,pub node:Option<u32>,pub message:String}
#[derive(Clone,Default)]
pub struct ElementState {pub visible:Option<bool>,pub enabled:Option<bool>,pub text:Option<String>,pub image:Option<String>}
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Action(Action),
    Visible(String, bool),
    Enabled(String, bool),
    Text(String, String),
    Image(String, String),
    Sound(String),
    Animate(String,AnimationClip),
}
impl Session {
    pub fn action_diagnostic(&mut self,page:&Page,target:Option<&str>,event:&Event,message:&str){
        let graphs:Vec<_>=page.graphs.iter().filter(|g|g.target.as_deref()==target&&&g.event==event).collect();
        let actions:Vec<_>=graphs.iter().flat_map(|g|g.nodes.iter().filter(|n|matches!(&n.op,Op::Action(a) if *a!=Action::None)).map(|n|(g.id.clone(),n.id))).collect();
        let (graph,node)=if actions.len()==1{(actions[0].0.clone(),Some(actions[0].1))}else{(graphs.first().map(|g|g.id.clone()).unwrap_or_default(),None)};
        self.last_error=Some(RuntimeDiagnostic{page:page.id.clone(),graph,node,message:message.into()});
    }
    /// A modal graph may decorate the decision, not bypass it with another
    /// game operation. Validate the entire result before committing locals.
    pub fn dispatch_confirmation(&mut self,page:&Page,target:Option<&str>,event:Event,fallback:Action)->Result<Vec<Effect>,String>{
        let activation=event==Event::Click;
        let mut trial=self.clone();let effects=match trial.dispatch(page,target,event.clone(),fallback){Ok(effects)=>effects,Err(error)=>{self.last_error=trial.last_error;return Err(error)}};
        let rejection=if effects.iter().any(|e|matches!(e,Effect::Action(a) if !matches!(a,Action::None|Action::Confirm|Action::Back))){Some("Une confirmation peut seulement confirmer ou annuler l’opération en attente")}else if !activation&&effects.iter().any(|e|matches!(e,Effect::Action(a) if *a!=Action::None)){Some("Une confirmation exige une activation explicite, jamais un survol ou une ouverture")}else{None};
        if let Some(message)=rejection{self.action_diagnostic(page,target,&event,message);return Err(message.into());}
        *self=trial;Ok(effects)
    }
    pub fn dispatch(&mut self,page:&Page,target:Option<&str>,event:Event,fallback:Action)->Result<Vec<Effect>,String>{
        self.last_error=None;
        let graphs:Vec<_>=page.graphs.iter().filter(|g|g.target.as_deref()==target&&g.event==event).collect();
        let mut trial=self.clone();let mut effects=vec![];
        if graphs.is_empty()&&event==Event::Click&&fallback!=Action::None{effects.push(Effect::Action(fallback));}
        for graph in graphs{match trial.execute(graph){Ok(result)=>effects.extend(result),Err(error)=>{self.last_error=trial.last_error.clone();if let Some(d)=&mut self.last_error{d.page=page.id.clone();}return Err(format!("{} : {error}",graph.id));}}}
        if effects.iter().filter(|e|matches!(e,Effect::Action(a) if *a!=Action::None)).count()>1{let message=diagnostic!("Plusieurs navigations pour une même activation : aucune action exécutée", "Multiple navigation actions for one activation: no action was executed").to_string();self.last_error=Some(RuntimeDiagnostic{page:page.id.clone(),graph:page.graphs.iter().find(|g|g.target.as_deref()==target&&g.event==event).map(|g|g.id.clone()).unwrap_or_default(),node:None,message:message.clone()});return Err(message)}
        *self=trial;Ok(effects)
    }
    pub fn apply_presentation(&mut self,page:&str,effect:&Effect){
        let id=match effect{Effect::Visible(id,_)|Effect::Enabled(id,_)|Effect::Text(id,_)|Effect::Image(id,_)=>id,_=>return};
        let state=self.presentation.entry((page.into(),id.clone())).or_default();match effect{Effect::Visible(_,v)=>state.visible=Some(*v),Effect::Enabled(_,v)=>state.enabled=Some(*v),Effect::Text(_,v)=>state.text=Some(v.clone()),Effect::Image(_,v)=>state.image=Some(v.clone()),_=>{}}
    }
    pub fn present(&self,source:&Document)->Document{let mut doc=source.clone();for ((page,id),state) in &self.presentation{if let Some(page)=doc.pages.iter().position(|p|&p.id==page){if let Some(e)=doc.find_element_mut(page,id){if let Some(v)=state.visible{e.visible=v}if let Some(v)=state.enabled{e.enabled=v}if let Some(v)=&state.text{e.text=v.clone();e.inherit_text=false;}if let Some(v)=&state.image{e.asset=Some(v.clone());e.overrides.image=Some(ResourceOverride::Path(v.clone()));}}}}for page in &mut doc.pages{for e in &mut page.elements{self.present_controls(e);}}for e in doc.components.values_mut(){self.present_controls(e);}doc}
    pub fn execute(&mut self, graph: &Graph) -> Result<Vec<Effect>, String> {
        self.last_error=None;self.cursor=None;
        let result=self.execute_inner(graph);
        if let Err(message)=&result{self.last_error=Some(RuntimeDiagnostic{page:String::new(),graph:graph.id.clone(),node:self.cursor,message:message.clone()});}
        result
    }
    fn execute_inner(&mut self, graph: &Graph) -> Result<Vec<Effect>, String> {
        let mut trial = self.variables.clone();
        let mut effects = Vec::new();
        let mut at = Some(graph.entry);
        for _ in 0..256 {
            let Some(id) = at else {
                self.variables = trial;
                return Ok(effects);
            };
            self.cursor=Some(id);
            let n = graph
                .nodes
                .iter()
                .find(|n| n.id == id)
                .ok_or_else(|| diagnostic!("Nœud de menu {id} absent", "Menu node {id} is missing"))?;
            self.cursor=Some(n.id);
            at = n.next;
            match &n.op {
                Op::Action(a) => effects.push(Effect::Action(a.clone())),
                Op::Visible { element, value } => {
                    effects.push(Effect::Visible(element.clone(), *value))
                }
                Op::Enabled { element, value } => {
                    effects.push(Effect::Enabled(element.clone(), *value))
                }
                Op::Text { element, value } => {
                    effects.push(Effect::Text(element.clone(), value.clone()))
                }
                Op::Image { element, path } => {
                    effects.push(Effect::Image(element.clone(), path.clone()))
                }
                Op::Sound {path}=>effects.push(Effect::Sound(path.clone())),
                Op::Animate{element,clip}=>{clip.validate()?;effects.push(Effect::Animate(element.clone(),clip.clone()));},
                Op::Set { variable, value } => {
                    if variable.starts_with("state.") {
                        return Err(diagnostic!("Les états du jeu sont en lecture seule", "Game state is read-only").into());
                    }
                    trial.insert(variable.clone(), value.clone());
                }
                Op::Branch {
                    variable,
                    equals,
                    otherwise,
                } => {
                    if trial.get(variable) != Some(equals) {
                        at = *otherwise;
                    }
                }
            }
        }
        if at.is_none(){self.variables=trial;Ok(effects)}else{Err(diagnostic!("Boucle d’événements interrompue (256 étapes)", "Event loop stopped (256 steps)").into())}
    }
}
impl Element {
    pub fn new(id: String, kind: Kind) -> Self {
        Self {
            name: id.clone(),
            id,
            kind,
            rect: [120.0, 120.0, 320.0, 64.0],
            anchors: [0.0; 4],
            text: "Élément".into(),
            asset: None,
            font: None,
            font_size: 28.0,
            normal: [0.13, 0.14, 0.17, 1.0],
            hover: [0.0, 0.40, 0.72, 1.0],
            pressed: [0.0, 0.27, 0.5, 1.0],
            disabled: [0.12, 0.12, 0.12, 0.6],
            foreground: [0.95, 0.95, 0.95, 1.0],
            visible: true,
            enabled: true,
            locked: false,
            focus_order: 0,
            action: Action::None,
            binding: None,
            local_control:None,
            visibility_binding: None,
            children: Vec::new(),
            style:None,overrides:StylePatch::default(),component:None,inherit_text:false,inherit_action:false,inherit_binding:false,layout_options:LayoutOptions::default(),list:ListOptions::default(),
            appearance:Appearance::default(),
        }
    }
    pub fn layout(&self, reference: [f32; 2], viewport: [f32; 2]) -> [f32; 4] {
        let scale = (viewport[0] / reference[0]).min(viewport[1] / reference[1]);
        [
            self.anchors[0] * viewport[0] + self.rect[0] * scale,
            self.anchors[1] * viewport[1] + self.rect[1] * scale,
            ((self.anchors[2] - self.anchors[0]) * viewport[0] + self.rect[2] * scale).max(0.0),
            ((self.anchors[3] - self.anchors[1]) * viewport[1] + self.rect[3] * scale).max(0.0),
        ]
    }
}
impl Document {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut doc: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if doc.version==1{doc.version=VERSION;fn migrate(es:&mut [Element]){for e in es{if e.kind==Kind::Image{e.layout_options.image_fit=ImageFit::Stretch;}migrate(&mut e.children);}}for page in &mut doc.pages{page.role=PageRole::from_id(&page.id);migrate(&mut page.elements);}}
        doc.validate()?;
        Ok(doc)
    }
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
    pub fn validate(&self) -> Result<(), String> {
        self.validate_authoring(&BTreeSet::new())
    }
    /// Temporary component canvases have no consuming page yet. Export must
    /// always use `validate`, after removing these authoring-only pages.
    pub fn validate_authoring(&self,workspaces:&BTreeSet<String>) -> Result<(), String> {
        self.validate_design()?;
        if self.version != VERSION {
            return Err(diagnostic!("Version de menus {} non prise en charge", "Unsupported menu version {}",
                self.version
            ));
        }
        if self.reference.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(diagnostic!("Dimensions de référence invalides", "Invalid reference dimensions").into());
        }
        if self.pages.is_empty() {
            return Err(diagnostic!("Le document doit contenir au moins une page", "The document must contain at least one page").into());
        }
        fn color(c: &Color) -> bool {
            c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        }
        let mut pages = BTreeSet::new();
        for p in &self.pages {
            if p.id.is_empty() || !pages.insert(&p.id) {
                return Err(diagnostic!("Identifiant de page vide ou dupliqué", "Empty or duplicate page ID").into());
            }
        }
        fn elements<'a>(
            list: &'a [Element],
            ids: &mut BTreeSet<&'a str>,
            pages: &BTreeSet<&String>,
            parent_size: [f32;2],
        ) -> Result<(), String> {
            for e in list {
                if e.id.is_empty() || !ids.insert(&e.id) {
                    return Err(diagnostic!("Élément dupliqué : {}", "Duplicate element: {}", e.id));
                }
                if [&e.normal, &e.hover, &e.pressed, &e.disabled, &e.foreground]
                    .iter()
                    .any(|c| !color(c))
                {
                    return Err(diagnostic!("Couleur invalide : {}", "Invalid color: {}", e.name));
                }
                if e.rect
                    .iter()
                    .chain(e.anchors.iter())
                    .any(|v| !v.is_finite())
                    || e.rect[2]+(e.anchors[2]-e.anchors[0])*parent_size[0] <= 0.0
                    || e.rect[3]+(e.anchors[3]-e.anchors[1])*parent_size[1] <= 0.0
                    || !e.font_size.is_finite()
                    || e.font_size <= 0.0
                {
                    return Err(diagnostic!("Dimensions invalides : {}", "Invalid dimensions: {}", e.name));
                }
                if e.anchors.iter().any(|v| !(0.0..=1.0).contains(v))
                    || e.anchors[2] < e.anchors[0]
                    || e.anchors[3] < e.anchors[1]
                {
                    return Err(diagnostic!("Ancres invalides : {}", "Invalid anchors: {}", e.name));
                }
                if let Action::OpenPage(id) = &e.action {
                    if !e.inherit_action && !pages.contains(id) {
                        return Err(diagnostic!("Page cible absente : {id}", "Target page is missing: {id}"));
                    }
                }
                let padding=e.layout_options.padding;
                let size=[(e.rect[2]+(e.anchors[2]-e.anchors[0])*parent_size[0]-padding[0]-padding[2]).max(0.0),(e.rect[3]+(e.anchors[3]-e.anchors[1])*parent_size[1]-padding[1]-padding[3]).max(0.0)];
                elements(&e.children, ids, pages,size)?;
            }
            Ok(())
        }
        for p in &self.pages {
            if !color(&p.background) {
                return Err(diagnostic!("Couleur de fond invalide : {}", "Invalid background color: {}", p.name));
            }
            let mut ids = BTreeSet::new();
            elements(&p.elements, &mut ids, &pages,self.reference)?;
            let page_index=self.pages.iter().position(|page|page.id==p.id).unwrap();
            let visual=self.layout_page(page_index,self.reference);
            if !workspaces.contains(&p.id){
                fn card_fields(doc:&Document,source:&Element,depth:usize)->Result<(),String>{
                    if depth>=64{return Err(diagnostic!("Composant trop profond", "Component nesting is too deep").into());}let e=doc.resolved_element(source);
                    if let Some(key)=e.binding.as_deref(){if (key.starts_with("save.")&&key!="save.page")||key.starts_with("gallery.")||key.starts_with("history.")||key=="choice.text"{return Err(diagnostic!("{} : la donnée {key} doit être placée dans un modèle de carte ou de ligne relié à sa liste", "{}: data binding {key} must be inside a card or row template connected to its list",e.name));}}
                    for child in &e.children{card_fields(doc,child,depth+1)?;}Ok(())
                }
                for element in &p.elements{card_fields(self,element,0)?;}
            }
            let page_actions:Vec<_>=visual.iter().map(|e|&e.action).chain(p.graphs.iter().flat_map(|g|&g.nodes).filter_map(|n|if let Op::Action(a)=&n.op{Some(a)}else{None})).filter_map(|a|if let Action::SavePage(target)=a{Some(target)}else{None}).collect();
            if !workspaces.contains(&p.id)&&(!page_actions.is_empty()||visual.iter().any(|e|e.binding.as_deref()==Some("save.page"))){
                let count=self.save_page_count(page_index).ok_or_else(||diagnostic!("{} : la pagination personnalisée nécessite une unique liste de sauvegardes sur la page", "{}: custom pagination requires exactly one save list on the page",p.name))?;
                if page_actions.iter().any(|target|matches!(target,SavePage::Number(n) if *n==0||*n>count)){return Err(diagnostic!("{} : numéro de page hors limites (1 à {count})", "{}: page number out of range (1 to {count})",p.name));}
            }
            let mut graph_ids = BTreeSet::new();
            for g in &p.graphs {
                if g.id.is_empty() || !graph_ids.insert(&g.id) {
                    return Err(diagnostic!("Identifiant de graphe vide ou dupliqué", "Empty or duplicate graph ID").into());
                }
                if g.target.as_ref().is_some_and(|t| !ids.contains(t.as_str())) {
                    return Err(diagnostic!("Cible absente du graphe {}", "Missing target in graph {}", g.id));
                }
                let nids: BTreeSet<_> = g.nodes.iter().map(|n| n.id).collect();
                if nids.len() != g.nodes.len() || !nids.contains(&g.entry) {
                    return Err(diagnostic!("Entrée ou identifiants invalides : {}", "Invalid entry or IDs: {}", g.id));
                }
                for n in &g.nodes {
                    if p.role==Some(PageRole::Confirm){if let Op::Action(action)=&n.op{
                        if *action!=Action::None&&(!matches!(action,Action::Confirm|Action::Back)||g.event!=Event::Click){return Err(diagnostic!("{} / nœud {} : une confirmation autorise seulement Confirmer ou Retour dans un événement clic", "{} / node {}: a confirmation only allows Confirm or Back in a click event",g.id,n.id));}
                    }}
                    if n.position.iter().any(|v| !v.is_finite()) {
                        return Err(diagnostic!("Position de nœud invalide", "Invalid node position").into());
                    }
                    if n.next.is_some_and(|id| !nids.contains(&id)) {
                        return Err(diagnostic!("Connexion absente : {}", "Missing connection: {}", g.id));
                    }
                    match &n.op {
                        Op::Set { variable, .. }
                            if variable.is_empty() || variable.starts_with("state.") =>
                        {
                            return Err(diagnostic!("Variable vide ou état du jeu en lecture seule", "Empty variable or read-only game state").into())
                        }
                        Op::Branch {
                            otherwise: Some(id),
                            ..
                        } if !nids.contains(id) => return Err(diagnostic!("Branche absente", "Missing branch").into()),
                        Op::Visible { element, .. }
                        | Op::Enabled { element, .. }
                        | Op::Text { element, .. }
                        | Op::Image { element, .. }
                        | Op::Animate { element, .. }
                            if !ids.contains(element.as_str()) =>
                        {
                            return Err(diagnostic!("Élément cible absent : {element}", "Target element is missing: {element}"))
                        }
                        Op::Action(Action::OpenPage(id)) if !pages.contains(id) => {
                            return Err(diagnostic!("Page cible absente : {id}", "Target page is missing: {id}"))
                        }
                        Op::Animate{clip,..}=>clip.validate().map_err(|e|diagnostic!("{} / nœud {} : {e}", "{} / node {}: {e}",g.id,n.id))?,
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
    pub fn validate_resources(&self,assets:&Path)->Result<(),String>{
        for path in self.resource_paths(){
            if path.is_empty()||Path::new(&path).components().any(|c|!matches!(c,std::path::Component::Normal(_))){return Err(diagnostic!("La ressource doit rester dans Assets : {path}", "Resource must remain inside Assets: {path}"));}
            let root=assets.canonicalize().map_err(|e|e.to_string())?;let file=assets.join(&path).canonicalize().map_err(|_|diagnostic!("Ressource absente : {path}", "Missing resource: {path}"))?;
            if !file.starts_with(root)||!file.is_file(){return Err(diagnostic!("Ressource invalide ou extérieure aux Assets : {path}", "Invalid resource or resource outside Assets: {path}"));}
        }Ok(())
    }
    pub fn validate_files(&self, assets: &Path, labels: &BTreeSet<String>) -> Result<(), String> {
        self.validate()?;
        self.validate_resources(assets)?;
        fn action(a: &Action, labels: &BTreeSet<String>) -> Result<(), String> {
            if let Action::StartScene(label) = a {
                if !labels.contains(label) {
                    return Err(diagnostic!("Label de menu absent : {label}", "Missing menu label: {label}"));
                }
            }
            Ok(())
        }
        fn list(doc:&Document,es: &[Element], assets: &Path, labels: &BTreeSet<String>) -> Result<(), String> {
            for e in es {
                let e=doc.resolved_element(e);
                for p in [&e.asset, &e.font].into_iter().flatten() {
                    if !assets.join(p).is_file() {
                        return Err(diagnostic!("Ressource de menu absente : {p}", "Missing menu resource: {p}"));
                    }
                }
                action(&e.action, labels)?;
                list(doc,&e.children, assets, labels)?;
            }
            Ok(())
        }
        for p in &self.pages {
            list(self,&p.elements, assets, labels)?;
            for g in &p.graphs {
                for n in &g.nodes {
                    match &n.op {
                        Op::Action(a) => action(a, labels)?,
                        Op::Image { path, .. } if !assets.join(path).is_file() => {
                            return Err(diagnostic!("Image absente : {path}", "Missing image: {path}"))
                        }
                        _ => {}
                    }
                }
            }
        }
        for component in self.components.values(){list(self,std::slice::from_ref(component),assets,labels)?;}
        Ok(())
    }
    pub fn defaults() -> Self {
        let mut pages = Vec::new();
        for (id, title, actions) in [
            (
                "title",
                "Menu principal",
                vec![
                    ("Continuer", Action::Continue),
                    ("Nouvelle partie", Action::NewGame),
                    ("Charger", Action::Load),
                    ("Réglages", Action::Settings),
                    ("Galerie", Action::Gallery),
                    ("Quitter", Action::Quit),
                ],
            ),
            (
                "pause",
                "Pause",
                vec![
                    ("Reprendre", Action::Resume),
                    ("Sauvegarder", Action::Save),
                    ("Charger", Action::Load),
                    ("Réglages", Action::Settings),
                    ("Historique", Action::History),
                    ("Quitter", Action::Quit),
                ],
            ),
            ("save", "Sauvegarder", vec![("Retour", Action::Back)]),
            ("load", "Charger", vec![("Retour", Action::Back)]),
            ("settings", "Réglages", vec![("Retour", Action::Back)]),
            ("gallery", "Galerie", vec![("Retour", Action::Back)]),
            ("history", "Historique", vec![("Retour", Action::Back)]),
            (
                "confirm",
                "Remplacer la partie en cours ?",
                vec![("Confirmer", Action::Confirm), ("Annuler", Action::Back)],
            ),
        ] {
            let mut p = Page {
                id: id.into(),
                name: title.into(),
                background: [0.035, 0.04, 0.055, 0.97],
                elements: vec![],
                graphs: vec![],
                role:PageRole::from_id(id),
            };
            let mut t = Element::new("heading".into(), Kind::Text);
            t.text = title.into();
            if id=="confirm"{t.binding=Some("confirmation.message".into());}
            t.rect = [100.0, 80.0, 1200.0, 90.0];
            t.font_size = 52.0;
            p.elements.push(t);
            for (i, (label, action)) in actions.into_iter().enumerate() {
                let mut e = Element::new(format!("button_{i}"), Kind::Button);
                e.text = label.into();
                e.rect = [100.0, 240.0 + i as f32 * 90.0, 420.0, 64.0];
                e.action = action;
                e.focus_order = i as i32;
                p.elements.push(e);
            }
            let dynamic = match id {
                "save" | "load" => Some(Kind::SaveList),
                "gallery" => Some(Kind::Gallery),
                "history" => Some(Kind::History),
                _ => None,
            };
            if let Some(kind) = dynamic {
                let mut e = Element::new("content".into(), kind);
                e.rect = [580.0, 200.0, 1220.0, 780.0];
                p.elements.push(e);
            }
            if id == "settings" {
                for (i, (label, key, kind)) in [
                    ("Musique", "music_volume", Kind::Slider),
                    ("Sons", "sfx_volume", Kind::Slider),
                    ("Vitesse du texte", "text_speed", Kind::Slider),
                    ("Écriture progressive", "typewriter", Kind::CheckBox),
                    ("Plein écran", "fullscreen", Kind::CheckBox),
                    ("Langue", "language", Kind::Select),
                ]
                .into_iter()
                .enumerate()
                {
                    let mut e = Element::new(key.into(), kind);
                    e.text = label.into();
                    e.binding = Some(key.into());
                    e.rect = [640.0, 220.0 + i as f32 * 100.0, 900.0, 70.0];
                    p.elements.push(e);
                }
            }
            pages.push(p);
        }
        let mut doc=Self {
            version: VERSION,
            reference: [1920.0, 1080.0],
            pages,
            styles:BTreeMap::new(),components:BTreeMap::new(),theme:ThemePreset::Sobre,
        };doc.apply_theme(ThemePreset::Sobre);doc
    }
    pub fn import_theme(source: &str) -> Result<Self, String> {
        Ok(Self::import_theme_report(source)?.document)
    }
}

#[cfg(test)]
mod tests {
    #[test]fn confirmation_graph_overrides_fallback_and_rejects_bypass_atomically(){
        use super::*;let mut page=Document::defaults().pages.remove(0);
        page.graphs=vec![Graph{id:"decision".into(),target:Some("button_0".into()),event:Event::Click,entry:0,nodes:vec![
            Node{id:0,position:[0.0;2],op:Op::Set{variable:"clicked".into(),value:true.into()},next:Some(1)},
            Node{id:1,position:[240.0,0.0],op:Op::Action(Action::NewGame),next:None},
        ]}];let mut session=Session::default();
        assert!(session.dispatch_confirmation(&page,Some("button_0"),Event::Click,Action::Confirm).is_err());assert!(session.variables.is_empty());
        assert_eq!(session.last_error.as_ref().unwrap().page,page.id);assert_eq!(session.last_error.as_ref().unwrap().node,Some(1));
        page.graphs[0].nodes[1].op=Op::Action(Action::Back);
        assert_eq!(session.dispatch_confirmation(&page,Some("button_0"),Event::Click,Action::Confirm).unwrap(),vec![Effect::Action(Action::Back)]);
        assert_eq!(session.variables["clicked"],true);
    }
    #[test]
    fn interaction_graph_replaces_simple_action(){let mut d=super::Document::defaults();let p=&mut d.pages[0];p.graphs.push(super::Graph{id:"click".into(),target:Some("button_0".into()),event:super::Event::Click,entry:0,nodes:vec![super::Node{id:0,position:[0.0;2],op:super::Op::Action(super::Action::Settings),next:None}]});let effects=super::Session::default().dispatch(p,Some("button_0"),super::Event::Click,super::Action::NewGame).unwrap();assert_eq!(effects,vec![super::Effect::Action(super::Action::Settings)]);}
    #[test]
    fn duplicate_navigation_is_atomic(){let d=super::Document::defaults();let mut p=d.pages[0].clone();for id in ["one","two"]{p.graphs.push(super::Graph{id:id.into(),target:None,event:super::Event::Open,entry:0,nodes:vec![super::Node{id:0,position:[0.0;2],op:super::Op::Action(super::Action::Settings),next:None}]});}assert!(super::Session::default().dispatch(&p,None,super::Event::Open,super::Action::None).is_err());}
    #[test]
    fn transient_presentation_preserves_source(){let d=super::Document::defaults();let before=d.clone();let mut s=super::Session::default();let page=&d.pages[0].id;let id=&d.pages[0].elements[0].id;s.apply_presentation(page,&super::Effect::Text(id.clone(),"Temporaire".into()));assert_eq!(s.present(&d).pages[0].elements[0].text,"Temporaire");assert_eq!(d,before);}
    use super::*;
    #[test]
    fn round_trip() {
        let d = Document::defaults();
        assert_eq!(Document::from_json(&d.to_json().unwrap()).unwrap(), d);
    }
    #[test]
    fn anchors() {
        let mut e = Element::new("e".into(), Kind::Button);
        e.anchors = [1.0, 1.0, 1.0, 1.0];
        e.rect = [-320.0, -64.0, 320.0, 64.0];
        assert_eq!(
            e.layout([1920.0, 1080.0], [1280.0, 720.0]),
            [1066.6666, 677.3333, 213.33334, 42.666668]
        );
    }
    #[test]
    fn invalid_link() {
        let mut d = Document::defaults();
        d.pages[0].elements[0].action = Action::OpenPage("missing".into());
        assert!(d.validate().is_err());
    }
    #[test]
    fn rejects_empty_pages_and_invalid_colors() {
        let mut d = Document::defaults();
        d.pages[0].background[0] = f32::NAN;
        assert!(d.validate().is_err());
        d.pages.clear();
        assert!(d.validate().is_err());
    }
    #[test]
    fn rejects_read_only_state_write() {
        let mut d = Document::defaults();
        d.pages[0].graphs.push(Graph {
            id: "write".into(),
            target: None,
            event: Event::Open,
            entry: 0,
            nodes: vec![Node {
                id: 0,
                position: [0.0; 2],
                op: Op::Set {
                    variable: "state.game_active".into(),
                    value: true.into(),
                },
                next: None,
            }],
        });
        assert!(d.validate().is_err());
    }
    #[test]
    fn migration_backup_preserves_original_bytes() {
        let root=std::env::temp_dir().join(format!("rvn-ui-migration-{}",std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path=root.join("menus.rvnui");
        let mut value=serde_json::to_value(Document::defaults()).unwrap();value["version"]=1.into();
        let original=serde_json::to_string_pretty(&value).unwrap();
        std::fs::write(&path,&original).unwrap();
        let migrated=Document::from_json(&original).unwrap();
        let saved=save_document(&path,&migrated,Some(&original)).unwrap();
        let backup=path.with_extension("rvnui.v1.bak");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(),original);
        save_document(&path,&migrated,Some(&saved)).unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(),original);
        std::fs::remove_file(backup).unwrap();std::fs::remove_file(path).unwrap();std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn file_conflict_is_preserved() {
        let root = std::env::temp_dir().join(format!("rvn-ui-save-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("menus.rvnui");
        let d = Document::defaults();
        std::fs::write(&path, "external").unwrap();
        assert!(save_document(&path, &d, None).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
        let saved = save_document(&path, &d, Some("external")).unwrap();
        assert_eq!(Document::from_json(&saved).unwrap(), d);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn cycle_is_atomic() {
        let g = Graph {
            id: "g".into(),
            target: None,
            event: Event::Click,
            entry: 1,
            nodes: vec![Node {
                id: 1,
                position: [0.0; 2],
                op: Op::Set {
                    variable: "x".into(),
                    value: 1.into(),
                },
                next: Some(1),
            }],
        };
        let mut s = Session::default();
        assert!(s.execute(&g).is_err());
        assert!(s.variables.is_empty());
        assert_eq!(s.last_error.as_ref().unwrap().node,Some(1));
    }
    #[test]fn execution_budget_accepts_exactly_256_steps_and_reports_overflow(){
        let mut graph=Graph{id:"budget".into(),target:None,event:Event::Open,entry:0,nodes:(0..256).map(|id|Node{id,position:[0.0;2],op:Op::Set{variable:"count".into(),value:id.into()},next:(id<255).then_some(id+1)}).collect()};
        let mut session=Session::default();session.execute(&graph).unwrap();assert_eq!(session.variables["count"],255);
        graph.nodes[255].next=Some(0);let mut page=Document::defaults().pages.remove(0);page.graphs=vec![graph];
        assert!(session.dispatch(&page,None,Event::Open,Action::None).is_err());
        let error=session.last_error.unwrap();assert_eq!(error.page,page.id);assert_eq!(error.graph,"budget");assert_eq!(error.node,Some(255));assert_eq!(session.variables["count"],255);
    }
}
