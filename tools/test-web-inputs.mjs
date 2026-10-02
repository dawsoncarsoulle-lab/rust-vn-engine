// Browser adapter unit test, independent of real-browser exported-game QA.
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';

const rust=readFileSync(new URL('../rvn_bevy/src/web_inputs.rs',import.meta.url),'utf8');
const source=rust.split(/inline_js\s*=\s*r#"/)[1].split('"#)]')[0].replaceAll('export function','function');
const faces=[];
class FontFace {
    constructor(family,resource){this.family=family;this.resource=resource;faces.push(this);}
    load(){return typeof this.resource==='string'&&this.resource.includes('missing.ttf')?Promise.reject(new Error('404')):Promise.resolve(this);}
}
const document={activeElement:null,listeners:{},fonts:{loaded:[],add(face){this.loaded.push(face);}},
    querySelector(selector){return selector==='canvas'?canvas:null;},
    createElement(tag){return new Element(tag);},
    addEventListener(name,listener){this.listeners[name]=listener;}};
class Element {
    constructor(tag){
        this.tag=tag;this.attributes={};this.listeners={};this.children=[];this.parent=null;this._value='';this.valueUpdates=0;
        this.style={_css:'',set cssText(value){this._css=value;this.display=/display:([^;]+)/.exec(value)?.[1];},get cssText(){return this._css;}};
    }
    get value(){return this._value;}
    set value(value){this._value=value;this.valueUpdates++;}
    setAttribute(key,value){this.attributes[key]=value;}
    addEventListener(name,listener){this.listeners[name]=listener;}
    appendChild(node){node.parent=this;this.children.push(node);}
    remove(){if(this.parent){this.parent.children.splice(this.parent.children.indexOf(this),1);this.parent=null;}if(document.activeElement===this)document.activeElement=null;}
    focus(){document.activeElement=this;this.listeners.focus?.();}
    blur(){if(document.activeElement===this)document.activeElement=null;}
    getBoundingClientRect(){return {left:10,top:20,width:960,height:540};}
}
document.body=new Element('body');const canvas=new Element('canvas');
const context=vm.createContext({document,FontFace,Map,Set,Array,JSON,encodeURIComponent});vm.runInContext(source,context);
const item=(font=null)=>({screen:'inventory',element:'search',value:'Potion',placeholder:'Search items',label:'Inventory search',enabled:true,key_target:null,layer:2,rect:[200,100,180,44],matrix:[1,0,0,1],clip:[[0,0],[180,0],[180,44],[0,44]],visible:true,font_size:24,padding:10,text_align:'center',font,color:'rgba(255,255,255,0.5)'});
const render=(fields,focus='search')=>context.rvn_render_inputs(JSON.stringify(fields),1920,1080,new Uint8Array([1,2,3]),'inventory',focus);
const settle=()=>new Promise(resolve=>setImmediate(resolve));

render([item('fonts/My Font.ttf')]);await settle();
const input=document.body.children[0];
assert.equal(faces.length,2,'Bundled and selected fonts are loaded once');
assert.equal(faces[1].resource,'url("assets/fonts/My%20Font.ttf")');
assert.ok(input.style.cssText.includes("font:12px 'RVN Interface 1',sans-serif"));
assert.ok(input.style.cssText.includes('text-align:center'));
assert.ok(input.style.cssText.includes('color:rgba(255,255,255,0.5)'));
assert.equal(document.activeElement,input);
assert.deepEqual(JSON.parse(context.rvn_input_events()),[],'Engine focus must not echo a new focus action');
const updates=input.valueUpdates;input.selectionStart=2;
render([item('fonts/My Font.ttf')]);await settle();
assert.equal(document.body.children[0],input,'A refresh retains IME and selection state');
assert.equal(input.valueUpdates,updates);assert.equal(input.selectionStart,2);assert.equal(faces.length,2);
input.listeners.compositionstart();input.value='Preedit';render([item('fonts/My Font.ttf')]);
assert.equal(input.value,'Preedit','Binding updates cannot overwrite active IME composition');
input.listeners.input({isComposing:true});assert.deepEqual(JSON.parse(context.rvn_input_events()),[]);
input.listeners.compositionend();assert.deepEqual(JSON.parse(context.rvn_input_events()),[{screen:'inventory',element:'search',kind:'change',value:'Preedit',key:null}]);
let prevented=false,stopped=false;
input.listeners.keydown({key:'Tab',shiftKey:false,isComposing:false,stopPropagation(){stopped=true;},preventDefault(){prevented=true;}});
assert.ok(prevented&&stopped);assert.equal(JSON.parse(context.rvn_input_events())[0].key,'__rvn_next_focus');
render([item('fonts/quote".otf')]);await settle();
assert.equal(faces[2].resource,'url("assets/fonts/quote%22.otf")','Asset names cannot inject CSS');
render([item('fonts/missing.ttf')]);await settle();
assert.throws(()=>context.rvn_input_events(),/Interface font could not be loaded: fonts\/missing.ttf/);
assert.deepEqual(JSON.parse(context.rvn_input_events()),[],'Failures are surfaced once, not endlessly replayed');
render([],'');assert.equal(document.body.children.length,0);assert.equal(context.rvn_input_focused(),false);
console.log('PASS: native browser input persistence, IME, focus, styles, custom font cache and load-error diagnostics');
