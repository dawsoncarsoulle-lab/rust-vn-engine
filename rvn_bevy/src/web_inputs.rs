//! Native browser text fields over the shared Bevy layout. Keeping the actual
//! editable node alive preserves browser selection, IME and clipboard behavior
//! when a binding refreshes the rest of an RVN screen.
use crate::{
    programmable_ui::{component_font_size, Control, Screens},
    resources::{ScriptErrorMessage, VnState},
};
use bevy::prelude::*;
use rvn_core::ui::UiInput;
use rvn_ui::programmable::{ComponentKind, ScreenEventKind};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const fields = new Map();
const customFonts = new Map();
let pending = [], failed = null, lastFocus = '', settingFocus = false, fontStarted = false;
const identity = (screen, element) => JSON.stringify([screen, element]);
function accessibilityShortcut(event) {
    if (event.key !== 'F8') return false;
    // Editing keeps ordinary keys local. Only the accessibility shortcut is
    // sent to Winit's canvas KeyboardInput listener, not the window DeviceEvent.
    event.preventDefault(); event.stopPropagation();
    const canvas = document.querySelector('canvas');
    if (canvas) {
        settingFocus = true; canvas.focus({preventScroll:true}); settingFocus = false;
        canvas.dispatchEvent(new KeyboardEvent(event.type, {key:event.key,code:event.code,location:event.location,repeat:event.repeat,altKey:event.altKey,ctrlKey:event.ctrlKey,metaKey:event.metaKey,shiftKey:event.shiftKey,isComposing:event.isComposing,bubbles:true,cancelable:true,composed:true}));
    }
    return true;
}
function queue(field, kind, value = null, key = null) {
    if (pending.length >= 128) { failed = 'Browser interface event queue exceeded 128 events'; return; }
    pending.push({screen: field.screen, element: kind === 'key' ? (field.keyTarget || field.element) : field.element, kind, value, key});
}
export function rvn_input_events() {
    if (failed) { const problem = failed; failed = null; pending = []; throw new Error(problem); }
    const events = pending; pending = []; return JSON.stringify(events);
}
export function rvn_input_focused() {
    return Array.from(fields.values()).some(field => field.node === document.activeElement);
}
export function rvn_render_inputs(description, width, height, font, focusScreen, focusElement) {
    const canvas = document.querySelector('canvas');
    if (!canvas) { if (description !== '[]') throw new Error('Game canvas is unavailable for browser inputs'); return; }
    if (!fontStarted) {
        fontStarted = true;
        const face = new FontFace('RVN Interface', font.slice().buffer);
        face.load().then(loaded => document.fonts.add(loaded)).catch(() => { failed = 'Bundled interface font could not be loaded'; });
        document.addEventListener('pointerdown', event => {
            if (event.target === canvas && rvn_input_focused()) document.activeElement.blur();
        }, true);
    }
    const rectangle = canvas.getBoundingClientRect(), sx = rectangle.width / width, sy = rectangle.height / height;
    const seen = new Set();
    for (const item of JSON.parse(description)) {
        const id = identity(item.screen, item.element); seen.add(id);
        let field = fields.get(id);
        if (!field) {
            const node = document.createElement('input');
            node.type = 'text'; node.autocomplete = 'off'; node.maxLength = 65536;
            node.setAttribute('data-rvn-screen', item.screen); node.setAttribute('data-rvn-element', item.element);
            field = {screen: item.screen, element: item.element, node, keyTarget: null, composing: false};
            fields.set(id, field); document.body.appendChild(node);
            node.addEventListener('focus', () => { if (!settingFocus) queue(field, 'focus'); });
            node.addEventListener('input', event => { if (!event.isComposing && !field.composing) queue(field, 'change', node.value); });
            node.addEventListener('compositionstart', () => { field.composing = true; });
            node.addEventListener('compositionend', () => { field.composing = false; queue(field, 'change', node.value); });
            node.addEventListener('keydown', event => {
                // Accessibility preferences remain reachable while editing.
                if (accessibilityShortcut(event)) return;
                // Native selection, editing, paste and IME remain browser-owned.
                // No keystroke in an input may advance or roll back the story.
                event.stopPropagation();
                if (field.keyTarget && !event.isComposing) queue(field, 'key', null, event.key);
                if (event.key === 'Tab') { event.preventDefault(); queue(field, 'key', null, event.shiftKey ? '__rvn_previous_focus' : '__rvn_next_focus'); }
            });
            node.addEventListener('keyup', event => { if (!accessibilityShortcut(event)) event.stopPropagation(); });
            node.addEventListener('pointerdown', event => event.stopPropagation());
            node.addEventListener('click', event => event.stopPropagation());
        }
        const node = field.node; field.keyTarget = item.key_target;
        let family = 'RVN Interface';
        if (item.font) {
            family = customFonts.get(item.font);
            if (!family) {
                if (customFonts.size >= 512) throw new Error('Browser interface font limit: 512 resources');
                family = `RVN Interface ${customFonts.size + 1}`;
                customFonts.set(item.font, family);
                // The same validated, project-relative asset loaded by Bevy.
                // Encoding every segment prevents names from becoming CSS.
                const url = 'assets/' + item.font.split('/').map(encodeURIComponent).join('/');
                const face = new FontFace(family, `url("${url}")`);
                face.load().then(loaded => document.fonts.add(loaded)).catch(() => { failed = `Interface font could not be loaded: ${item.font}`; });
            }
        }
        // Never reset selection on a normal binding echo or during preedit.
        if (!field.composing && node.value !== item.value) node.value = item.value;
        node.placeholder = item.placeholder; node.disabled = !item.enabled;
        node.setAttribute('aria-label', item.label);
        node.tabIndex = -1; // The shared authored order is used by the engine.
        const [x,y,w,h] = item.rect, [a,b,c,d] = item.matrix;
        const hidden = w <= 0 || h <= 0 || !item.visible;
        const clip = item.clip.map(([px,py])=>`${px*sx}px ${py*sy}px`).join(',');
        // The DOM and canvas use the same affine basis, including animated
        // ancestors. Clipping is transformed back into the field's local space.
        node.style.cssText = `position:fixed;box-sizing:border-box;left:${rectangle.left+(x-w/2)*sx}px;top:${rectangle.top+(y-h/2)*sy}px;width:${w*sx}px;height:${h*sy}px;z-index:${10000+item.layer};display:${hidden?'none':'block'};transform-origin:center;transform:matrix(${a},${b*sy/sx},${c*sx/sy},${d},0,0);border:0;outline:none;border-radius:5px;background:transparent;color:${item.color};font:${item.font_size*sy}px '${family}',sans-serif;text-align:${item.text_align};padding:${item.padding*sy}px;clip-path:polygon(${clip});`;
    }
    for (const [id,field] of fields) { if (!seen.has(id)) { field.node.remove(); fields.delete(id); } }
    const requested = identity(focusScreen, focusElement), field = fields.get(requested);
    if (requested !== lastFocus) {
        if (field && !field.node.disabled && field.node.style.display !== 'none') {
            settingFocus = true; field.node.focus({preventScroll:true}); settingFocus = false; lastFocus = requested;
        } else if (!field) {
            if (rvn_input_focused()) { document.activeElement.blur(); canvas.focus({preventScroll:true}); }
            lastFocus = requested;
        }
    }
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn rvn_render_inputs(
        description: &str,
        width: f32,
        height: f32,
        font: &[u8],
        focus_screen: &str,
        focus_element: &str,
    ) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    fn rvn_input_events() -> Result<String, JsValue>;
    fn rvn_input_focused() -> bool;
}

pub fn focused() -> bool {
    rvn_input_focused()
}
pub fn events() -> Result<Vec<UiInput>, String> {
    #[derive(serde::Deserialize)]
    struct BrowserEvent {
        screen: String,
        element: String,
        kind: ScreenEventKind,
        value: Option<serde_json::Value>,
        key: Option<String>,
    }
    let text = rvn_input_events().map_err(|error| format!("Browser input: {error:?}"))?;
    let events: Vec<BrowserEvent> =
        serde_json::from_str(&text).map_err(|error| format!("Browser input event: {error}"))?;
    events
        .into_iter()
        .map(|event| {
            Ok(UiInput {
                screen: event.screen,
                element: event.element,
                kind: event.kind,
                value: event
                    .value
                    .as_ref()
                    .map(rvn_core::ui::value_from_json)
                    .transpose()
                    .map_err(|error| format!("Browser input value: {error}"))?,
                key: event.key,
            })
        })
        .collect()
}

pub fn render(
    screens: Res<Screens>,
    windows: Query<&Window>,
    controls: Query<(
        &Control,
        &Node,
        &GlobalTransform,
        Option<&bevy::ui::CalculatedClip>,
        Option<&Children>,
    )>,
    texts: Query<&Text>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    mut initialized: Local<bool>,
    accessibility: Res<crate::accessibility::Accessibility>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let mut fields = Vec::new();
    let scale = (window.width() / 1920.0)
        .min(window.height() / 1080.0)
        .max(0.25);
    if screens.active && !accessibility.open {
        for (control, node, transform, clip, children) in &controls {
            if control.option.is_some() {
                continue;
            }
            let Some(view) = screens
                .views
                .iter()
                .find(|view| view.name == control.screen)
            else {
                continue;
            };
            let Some(component) = view
                .root
                .find(&control.element)
                .filter(|component| component.kind == ComponentKind::Input)
            else {
                continue;
            };
            let matrix = transform.compute_matrix();
            if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 {
                continue;
            }
            let bounds = Rect::new(0.0, 0.0, window.width(), window.height());
            let clipped = bounds.intersect(clip.map_or(bounds, |clip| clip.clip));
            let corners = [
                clipped.min,
                Vec2::new(clipped.max.x, clipped.min.y),
                clipped.max,
                Vec2::new(clipped.min.x, clipped.max.y),
            ];
            let polygon: Vec<_> = corners
                .into_iter()
                .map(|corner| {
                    let local = matrix
                        .inverse()
                        .transform_point3(corner.extend(transform.translation().z))
                        .truncate()
                        + node.size() * 0.5;
                    [local.x, local.y]
                })
                .collect();
            let enabled = view.root.available(&component.id)
                && !screens.views.iter().any(|other| {
                    other.modal && (other.layer, other.order) > (view.layer, view.order)
                });
            // DOM fields cannot sit behind a modal painted in the canvas.
            // Inactive fields use the canvas's read-only label until uncovered.
            if !enabled {
                continue;
            }
            let key_target = if component.events.contains_key(&ScreenEventKind::Key) {
                Some(component.id.as_str())
            } else if view.root.events.contains_key(&ScreenEventKind::Key) {
                Some(view.root.id.as_str())
            } else {
                None
            };
            let foreground = children
                .and_then(|children| {
                    children.iter().find_map(|child| {
                        texts
                            .get(*child)
                            .ok()
                            .and_then(|text| text.sections.first())
                            .map(|section| section.style.color.to_srgba())
                    })
                })
                .unwrap_or_else(|| {
                    Color::srgba(
                        component.foreground[0],
                        component.foreground[1],
                        component.foreground[2],
                        component.foreground[3],
                    )
                    .to_srgba()
                });
            fields.push(serde_json::json!({"screen":view.name,"element":component.id,"value":component.value.as_str().unwrap_or_default(),"placeholder":component.placeholder,"label":component.accessible_label.as_deref().filter(|label|!label.is_empty()).unwrap_or(if component.placeholder.is_empty(){&component.id}else{&component.placeholder}),"enabled":enabled,"key_target":key_target,"layer":screens.views.iter().position(|other|other.name==view.name).unwrap_or(0),"rect":[transform.translation().x,transform.translation().y,node.size().x,node.size().y],"matrix":[matrix.x_axis.x,matrix.x_axis.y,matrix.y_axis.x,matrix.y_axis.y],"clip":polygon,"visible":clipped.width()>0.0&&clipped.height()>0.0,"font_size":component_font_size(component.font_size,scale),"padding":if component.padding==0.0{10.0*scale}else{component.padding*scale},"text_align":component.text_align,"font":view.root.effective_font(&component.id),"color":format!("rgba({},{},{},{})",foreground.red*255.0,foreground.green*255.0,foreground.blue*255.0,foreground.alpha)}));
        }
    }
    let (screen, element) = screens
        .keyboard_focus
        .as_ref()
        .map(|(screen, element)| (screen.as_str(), element.as_str()))
        .unwrap_or(("", ""));
    let font: &[u8] = if *initialized {
        &[]
    } else {
        include_bytes!("../resources/DejaVuSans.ttf")
    };
    // The browser field follows the scaled Bevy font instead of obscuring it
    // with a smaller, unscaled native input when text size is increased.
    for field in &mut fields {
        if let Some(size) = field.get("font_size").and_then(|size| size.as_f64()) {
            field["font_size"] =
                serde_json::json!(size * f64::from(accessibility.settings.text_scale));
        }
    }
    if let Err(problem) = rvn_render_inputs(
        &serde_json::to_string(&fields).unwrap(),
        window.width(),
        window.height(),
        font,
        screen,
        element,
    ) {
        error.0 = format!("Browser interface input could not be displayed: {problem:?}");
        next.set(VnState::Error);
    } else {
        *initialized = true;
    }
}
