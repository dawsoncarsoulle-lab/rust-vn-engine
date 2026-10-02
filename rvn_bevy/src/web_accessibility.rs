//! A semantic browser tree complements the rendered canvas. Screen-reader
//! actions travel through the same AccessKit event path as native platforms.
use bevy::a11y::{
    accesskit::{Action, ActionData, NodeId, Role, Toggled},
    AccessibilityNode, ActionRequest,
};
use bevy::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const semanticNodes=new Map();
let semanticEvents=[],semanticRoot=null,semanticSettingFocus=false,semanticLastFocus='',semanticCanvasAria=null;
function semanticCanvasFocus(entry){
    const canvas=document.querySelector('canvas');if(!canvas)return;
    // A freshly focused native RVN input may precede its binding echo by one
    // frame. Never steal selection/IME focus while that event is in transit.
    if(document.activeElement?.matches('[data-rvn-screen][data-rvn-element]'))return;
    if(!semanticCanvasAria){semanticCanvasAria={canvas,role:canvas.getAttribute('role'),label:canvas.getAttribute('aria-label'),controls:canvas.getAttribute('aria-controls')};}
    canvas.setAttribute('role','application');canvas.setAttribute('aria-label',entry.node.getAttribute('aria-label')||'');canvas.setAttribute('aria-controls',entry.node.id);
    // Keep real pointer capture and keyboard events on Winit's graphic canvas.
    // Its semantic proxy is a named group, never a hidden competing button.
    if(document.activeElement!==canvas){semanticSettingFocus=true;canvas.focus({preventScroll:true});semanticSettingFocus=false;}
}
function semanticCanvasClear(){
    if(!semanticCanvasAria)return;const {canvas,role,label,controls}=semanticCanvasAria;
    for(const [name,value]of [['role',role],['aria-label',label],['aria-controls',controls]]){if(value===null)canvas.removeAttribute(name);else canvas.setAttribute(name,value);}
    semanticCanvasAria=null;
}
function semanticQueue(id,action,value=null){
    if(semanticEvents.length<128)semanticEvents.push({id,action,value});
}
function semanticEngineShortcut(event){
    // Winit's WindowEvent::KeyboardInput listener is on the graphic canvas;
    // bubbling to window only creates DeviceEvent, not Bevy ButtonInput. Relay
    // reserved press/release there while ordinary control keys stay local.
    if(['Escape','F5','F6','F8','F9','F10'].includes(event.key)){
        event.preventDefault();
        const canvas=document.querySelector('canvas');
        if(canvas&&event.target!==canvas){
            event.stopPropagation();
            semanticSettingFocus=true;canvas.focus({preventScroll:true});semanticSettingFocus=false;
            canvas.dispatchEvent(new KeyboardEvent(event.type,{key:event.key,code:event.code,location:event.location,repeat:event.repeat,altKey:event.altKey,ctrlKey:event.ctrlKey,metaKey:event.metaKey,shiftKey:event.shiftKey,isComposing:event.isComposing,bubbles:true,cancelable:true,composed:true}));
        }
        return true;
    }
    return false;
}
export function rvn_semantic_events(){const events=semanticEvents;semanticEvents=[];return JSON.stringify(events);}
export function rvn_semantic_render(description,focused,language){
    if(!semanticRoot){
        semanticRoot=document.createElement('section');semanticRoot.setAttribute('aria-label','rust-VN');
        semanticRoot.style.cssText='position:fixed;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap';
        document.body.appendChild(semanticRoot);
    }
    semanticRoot.lang=language;
    const seen=new Set();let position=0;
    for(const item of JSON.parse(description)){
        const identity=item.key||item.id;seen.add(identity);
        let entry=semanticNodes.get(identity);
        if(!entry||entry.role!==item.role){
            entry?.node.remove();
            const node=document.createElement(item.role==='select'?'select':item.role==='checkbox'||item.role==='slider'?'input':item.role==='button'?'button':'span');
            if(item.role==='checkbox')node.type='checkbox';if(item.role==='slider')node.type='range';
            node.setAttribute('data-rvn-accessibility',item.id);
            entry={node,role:item.role,id:item.id,key:identity};semanticNodes.set(identity,entry);
            node.addEventListener('focus',()=>{if(!semanticSettingFocus){semanticQueue(entry.id,'focus');if(entry.role==='canvas')semanticCanvasFocus(entry);}});
            if(item.role==='button')node.addEventListener('click',()=>semanticQueue(entry.id,'default'));
            if(item.role==='canvas')node.addEventListener('click',()=>{if(entry.activate)semanticQueue(entry.id,'default');});
            if(item.role==='checkbox')node.addEventListener('change',()=>semanticQueue(entry.id,'default'));
            if(item.role==='select'||item.role==='slider')node.addEventListener('change',()=>semanticQueue(entry.id,'value',node.value));
            node.addEventListener('keydown',event=>{if(!semanticEngineShortcut(event))event.stopPropagation();if(event.key==='Tab'&&(event.shiftKey?entry.previous:entry.next)){event.preventDefault();semanticQueue(event.shiftKey?entry.previous:entry.next,'focus');}});
            node.addEventListener('keyup',event=>{if(!semanticEngineShortcut(event))event.stopPropagation();});
            semanticRoot.appendChild(node);
        }
        const node=entry.node;
        if(semanticRoot.children[position]!==node){const active=document.activeElement===node;semanticSettingFocus=true;semanticRoot.insertBefore(node,semanticRoot.children[position]||null);if(active)node.focus({preventScroll:true});semanticSettingFocus=false;}position++;
        entry.id=item.id;entry.next=item.next;entry.previous=item.previous;entry.activate=item.activate;node.setAttribute('data-rvn-accessibility',item.id);
        node.id='rvn-semantic-'+item.id;
        node.hidden=item.hidden;node.disabled=item.disabled;
        node.setAttribute('aria-disabled',String(item.disabled));
        node.setAttribute('aria-label',item.name);
        if(item.role==='image')node.setAttribute('role','img');
        if(item.role==='canvas'){node.setAttribute('role','group');node.tabIndex=item.disabled||item.hidden?-1:0;}
        if(item.role==='select'){
            const signature=JSON.stringify(item.options);
            if(entry.options!==signature){entry.options=signature;node.replaceChildren(...item.options.map(option=>{const child=document.createElement('option');child.value=option.value;child.textContent=option.label;return child;}));}
            if(node.value!==item.value)node.value=item.value;
        }else if(item.role==='checkbox')node.checked=item.checked;
        else if(item.role==='slider'){node.min=item.min;node.max=item.max;node.step=item.step;node.value=item.numeric;}
        else node.textContent=item.name;
        if(item.role==='text')node.setAttribute('aria-live','polite');
    }
    for(const [id,entry]of semanticNodes){if(!seen.has(id)){entry.node.remove();semanticNodes.delete(id);}}
    const currentEntry=Array.from(semanticNodes.values()).find(entry=>entry.id===focused),current=currentEntry?.node;
    // Focus follows authored navigation without resetting an editable DOM
    // field, stealing pointer focus, or repeatedly restarting announcements.
    const identity=currentEntry?.key||'';
    if(currentEntry?.role==='canvas'&&current&&!current.disabled&&!current.hidden){
        semanticLastFocus=identity;semanticCanvasFocus(currentEntry);
    }else{
        semanticCanvasClear();
        if(identity!==semanticLastFocus){if(current&&!current.disabled&&!current.hidden){semanticLastFocus=identity;if(document.activeElement!==current){semanticSettingFocus=true;current.focus({preventScroll:true});semanticSettingFocus=false;}}else if(!focused){semanticLastFocus='';}}
    }
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn rvn_semantic_events() -> Result<String, JsValue>;
    #[wasm_bindgen(catch)]
    fn rvn_semantic_render(description: &str, focused: &str, language: &str)
        -> Result<(), JsValue>;
}
#[derive(serde::Deserialize)]
struct Request {
    id: String,
    action: String,
    value: Option<String>,
}
pub(super) fn input(
    mut output: EventWriter<ActionRequest>,
    mut access: ResMut<crate::accessibility::Accessibility>,
) {
    let requests = rvn_semantic_events()
        .map_err(|error| format!("Browser accessibility: {error:?}"))
        .and_then(|json| {
            serde_json::from_str::<Vec<Request>>(&json).map_err(|error| error.to_string())
        });
    match requests {
        Ok(requests) => {
            for request in requests.into_iter().take(128) {
                let Ok(id) = request.id.parse::<u64>() else {
                    continue;
                };
                let action = match request.action.as_str() {
                    "focus" => Action::Focus,
                    "default" => Action::Default,
                    "value" => Action::SetValue,
                    _ => continue,
                };
                if request
                    .value
                    .as_ref()
                    .is_some_and(|value| value.len() > 65_536)
                {
                    continue;
                }
                output.send(ActionRequest(bevy::a11y::accesskit::ActionRequest {
                    target: NodeId(id),
                    action,
                    data: request
                        .value
                        .map(|value| ActionData::Value(value.into_boxed_str())),
                }));
            }
        }
        Err(problem) => access.browser_error(problem),
    }
}
pub(super) fn render(
    nodes: Query<(
        Entity,
        &AccessibilityNode,
        Option<&InheritedVisibility>,
        Option<&crate::programmable_ui::Control>,
    )>,
    targets: Query<&crate::composed_motion::InterfaceTarget>,
    choices: Query<&crate::components::ChoiceButton>,
    ancestors: Query<(
        Option<&Parent>,
        Option<&crate::menu_documents::VisualNode>,
        Option<&crate::menu_documents::ChoicesRoot>,
        Option<&crate::components::ChoiceContainer>,
    )>,
    state: Res<crate::resources::VnRenderState>,
    screens: Res<crate::programmable_ui::Screens>,
    focus: Res<bevy::a11y::Focus>,
    locale: Res<crate::systems::settings_menu::Settings>,
    mut access: ResMut<crate::accessibility::Accessibility>,
) {
    let mut descriptions = Vec::new();
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    let order: Vec<_> = screens
        .views
        .iter()
        .skip(modal)
        .flat_map(|view| {
            view.root
                .focus_order()
                .into_iter()
                .map(|id| (view.name.clone(), id))
        })
        .collect();
    let entity_for = |name: &str, id: &str| {
        nodes.iter().find_map(|(entity, _, _, control)| {
            control
                .filter(|control| {
                    control.option.is_none() && control.screen == name && control.element == id
                })
                .map(|_| entity.to_bits().to_string())
        })
    };
    for (entity, node, visible, control) in &nodes {
        let built = node.0.clone().build();
        // Real DOM text fields already expose selection, IME and clipboard.
        // Never add a second, competing text input for the same component.
        let role = match built.role() {
            Role::Button => "button",
            Role::CheckBox => "checkbox",
            Role::Slider => "slider",
            Role::ComboBox => "select",
            Role::StaticText => "text",
            Role::Image => "image",
            Role::Canvas => "canvas",
            _ => continue,
        };
        let options=control.and_then(|control|screens.views.iter().find(|view|view.name==control.screen).and_then(|view|view.root.find(&control.element))).map(|component|component.options.iter().enumerate().map(|(index,value)|serde_json::json!({"value":value,"label":component.option_labels.get(index).unwrap_or(value)})).collect::<Vec<_>>()).unwrap_or_default();
        let adjacent = |back: bool| {
            control
                .and_then(|control| {
                    order.iter().position(|(screen, id)| {
                        screen == &control.screen && id == &control.element
                    })
                })
                .and_then(|index| {
                    let index = if back {
                        (index + order.len() - 1) % order.len()
                    } else {
                        (index + 1) % order.len()
                    };
                    entity_for(&order[index].0, &order[index].1)
                })
        };
        let ordering = control
            .and_then(|control| {
                order
                    .iter()
                    .position(|(screen, id)| screen == &control.screen && id == &control.element)
            })
            .map(|index| 100_000.0 + index as f64)
            .unwrap_or_else(|| built.bounds().map_or(0.0, |bounds| bounds.y0));
        // Menu appearance/font/viewport changes rebuild Bevy choice entities.
        // Their DOM identity must follow the logical list and choice instead:
        // destroying the focused proxy otherwise silently focuses BODY. Keep
        // custom and hidden legacy lists distinct, and include the authored
        // ancestor path plus option context to avoid cross-list collisions.
        let choice_key = choices.get(entity).ok().and_then(|choice| {
            let mut cursor = entity;
            let mut path = Vec::new();
            for _ in 0..64 {
                let Ok((parent, visual, custom, legacy)) = ancestors.get(cursor) else {
                    break;
                };
                if let Some(visual) = visual {
                    path.push((&visual.page, &visual.id));
                }
                if custom.is_some() || legacy.is_some() {
                    return Some(
                        serde_json::json!([
                            "choice",
                            if custom.is_some() { "custom" } else { "legacy" },
                            path,
                            state.choice_options,
                            choice.0,
                            built.name().unwrap_or_default()
                        ])
                        .to_string(),
                    );
                }
                let Some(parent) = parent else {
                    break;
                };
                cursor = parent.get();
            }
            None
        });
        let key = control
            .map(|control| {
                serde_json::json!(["control", control.screen, control.element, control.option])
                    .to_string()
            })
            .or(choice_key)
            .or_else(|| {
                targets.get(entity).ok().map(|target| {
                    serde_json::json!(["component", target.screen, target.element]).to_string()
                })
            })
            .unwrap_or_else(|| entity.to_bits().to_string());
        descriptions.push(serde_json::json!({"id":entity.to_bits().to_string(),"key":key,"role":role,"name":built.name().unwrap_or_default(),"value":built.value().unwrap_or_default(),"activate":built.supports_action(Action::Default),"disabled":built.is_disabled(),"hidden":built.is_hidden()||visible.is_some_and(|visible|!visible.get()),"checked":built.toggled()==Some(Toggled::True),"numeric":built.numeric_value().unwrap_or(0.0),"min":built.min_numeric_value().unwrap_or(0.0),"max":built.max_numeric_value().unwrap_or(1.0),"step":built.numeric_value_step().unwrap_or(0.01),"options":options,"next":adjacent(false),"previous":adjacent(true),"order":ordering}));
    }
    descriptions.sort_by(|a, b| {
        a["order"]
            .as_f64()
            .unwrap_or_default()
            .total_cmp(&b["order"].as_f64().unwrap_or_default())
    });
    let focused = focus
        .0
        .map(|entity| entity.to_bits().to_string())
        .unwrap_or_default();
    if let Err(problem) = rvn_semantic_render(
        &serde_json::to_string(&descriptions).unwrap(),
        &focused,
        &locale.language,
    ) {
        access.browser_error(format!("Browser accessibility: {problem:?}"));
    }
}
