// Adapter unit test. Real browser exports are verified separately.
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const rust=readFileSync(new URL('../rvn_bevy/src/web_accessibility.rs',import.meta.url),'utf8');
const source=rust.split('inline_js=r#"')[1].split('"#)]')[0].replaceAll('export function','function');
const document={activeElement:null,body:null,createElement(tag){return new Element(tag);}};
class Element {
    constructor(tag){this.tag=tag;this.style={};this.attributes={};this.children=[];this.listeners={};this.parent=null;this.value='';}
    setAttribute(key,value){this.attributes[key]=value;}
    addEventListener(name,listener){this.listeners[name]=listener;}
    appendChild(node){node.remove();node.parent=this;this.children.push(node);}
    insertBefore(node,next){node.remove();node.parent=this;const index=this.children.indexOf(next);this.children.splice(index<0?this.children.length:index,0,node);}
    remove(){if(this.parent){this.parent.children.splice(this.parent.children.indexOf(this),1);this.parent=null;}}
    focus(){document.activeElement=this;this.listeners.focus?.();}
    replaceChildren(...nodes){for(const node of [...this.children])node.remove();for(const node of nodes)this.appendChild(node);}
}
document.body=new Element('body');
const context=vm.createContext({document,Map,Set,Array,JSON});vm.runInContext(source,context);
const description=(id,value='Dark')=>[{id,key:'inventory/theme',role:'select',name:'Reading theme',value,options:[{value:'Dark',label:'Dark'},{value:'White',label:'White'}],hidden:false,disabled:false,next:'99',previous:'98'}];
context.rvn_semantic_render(JSON.stringify(description('1')),'1','en');
const node=document.body.children[0].children[0];assert.equal(document.activeElement,node);
node.value='White';node.listeners.change();assert.deepEqual(JSON.parse(context.rvn_semantic_events()),[{id:'1',action:'value',value:'White'}]);
context.rvn_semantic_render(JSON.stringify(description('2','White')),'2','fr');
assert.equal(document.body.children[0].children[0],node,'A binding refresh must retain the DOM control');
assert.equal(document.activeElement,node);assert.equal(node.value,'White');assert.equal(node.attributes['data-rvn-accessibility'],'2');
node.listeners.change();assert.equal(JSON.parse(context.rvn_semantic_events())[0].id,'2','Events must target the replacement Bevy entity');
let prevented=false;node.listeners.keydown({key:'Tab',shiftKey:false,stopPropagation(){},preventDefault(){prevented=true;}});
assert.ok(prevented);assert.equal(JSON.parse(context.rvn_semantic_events())[0].id,'99');
context.rvn_semantic_render(JSON.stringify([{id:'3',key:'journal/quest',role:'text',name:'Archive unlocked',hidden:false,disabled:false}]),'','en');
const text=document.body.children[0].children[0];assert.equal(text.textContent,'Archive unlocked');assert.equal(text.attributes['aria-live'],'polite');
context.rvn_semantic_render('[]','','en');assert.equal(document.body.children[0].children.length,0);
console.log('PASS: stable browser semantics, current-entity actions, focus, values, live text and disposal');
