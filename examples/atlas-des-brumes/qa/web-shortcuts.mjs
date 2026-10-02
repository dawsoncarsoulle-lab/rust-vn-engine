// Regression for the shipped semantic bridge, executed in a real browser DOM.
// It extracts the actual inline adapter instead of copying its event logic.
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {createRequire} from 'node:module';
import {homedir} from 'node:os';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url);
const modules=process.env.RVN_QA_NODE_MODULES||join(homedir(),'.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules');
let playwright;try{playwright=require('playwright');}catch{playwright=require(join(modules,'playwright'));}
const file=new URL('../../../rvn_bevy/src/web_accessibility.rs',import.meta.url);
const adapter=readFileSync(file,'utf8').match(/inline_js=r#"\n([\s\S]*?)\n"#/)[1].replace(/^export /gm,'');
const inputAdapter=readFileSync(new URL('../../../rvn_bevy/src/web_inputs.rs',import.meta.url),'utf8').match(/inline_js = r#"\n([\s\S]*?)\n"#/)[1].replace(/^export /gm,'');
const font=Array.from(readFileSync(new URL('../../../rvn_bevy/resources/DejaVuSans.ttf',import.meta.url)));
for(const name of ['chrome','firefox']){
    const browser=name==='chrome'
        ?await playwright.chromium.launch({headless:true,executablePath:process.env.RVN_CHROME_BIN||'/usr/bin/google-chrome',args:['--no-sandbox']})
        :await playwright.firefox.launch({headless:true});
    try{
        const page=await browser.newPage();
        await page.setContent('<!doctype html><canvas tabindex="0"></canvas><input data-rvn-screen="atlas" data-rvn-element="atlas_first_name" value="Alix">');
        await page.addScriptTag({content:adapter+`\nwindow.bridge={render:rvn_semantic_render,events:rvn_semantic_events};`});
        await page.evaluate(()=>{
            window.keys=[];window.canvasKeys=[];
            // Match Winit's actual WindowEvent keyboard target and its default
            // suppression, not merely the window DeviceEvent listener.
            for(const type of ['keydown','keyup'])document.querySelector('canvas').addEventListener(type,event=>{event.preventDefault();window.canvasKeys.push({type,key:event.key,code:event.code,location:event.location,repeat:event.repeat,ctrl:event.ctrlKey,alt:event.altKey,shift:event.shiftKey,meta:event.metaKey,target:event.target.tagName});});
            for(const type of ['keydown','keyup'])window.addEventListener(type,event=>window.keys.push({type,key:event.key,prevented:event.defaultPrevented,target:event.target.tagName}));
            const node={id:'1',key:'choice',role:'button',name:'Voir l’ouverture animée',hidden:false,disabled:false,next:'2',previous:'2'};
            window.bridge.render(JSON.stringify([node,{...node,id:'2',key:'other',name:'Rouvrir l’Atlas',next:'1',previous:'1'}]),'1','fr');
        });
        assert.equal(await page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'Voir l’ouverture animée');
        // A real appearance/font/viewport update may replace the Bevy Entity,
        // but not the logical choice. Extracted shipped JS must retain the DOM
        // node and its focus while all event closures use the new Entity id.
        await page.evaluate(()=>{
            window.originalChoice=document.querySelector('[data-rvn-accessibility="1"]');
            window.originalOther=document.querySelector('[data-rvn-accessibility="2"]');
            window.bridge.events();
            const node={id:'11',key:'choice',role:'button',name:'Voir l’ouverture animée',hidden:false,disabled:false,next:'2',previous:'2'};
            const legacy={...node,id:'31',key:'legacy-choice',hidden:true,disabled:true};
            window.bridge.render(JSON.stringify([legacy,node,{...node,id:'2',key:'other',name:'Rouvrir l’Atlas',next:'11',previous:'11'}]),'','fr');
        });
        assert.deepEqual(await page.evaluate(()=>({same:document.querySelector('[data-rvn-accessibility="11"]')===window.originalChoice,focused:document.activeElement===window.originalChoice,other:document.querySelector('[data-rvn-accessibility="2"]')===window.originalOther})),{same:true,focused:true,other:true},`${name}: replacing choice Entity must preserve the same focused DOM node without colliding with hidden legacy controls`);
        await page.locator('[data-rvn-accessibility="11"]').evaluate(node=>node.click());
        assert.deepEqual(await page.evaluate(()=>JSON.parse(window.bridge.events())),[{id:'11',action:'default',value:null}],`${name}: retained click closure must address the current Entity`);
        await page.keyboard.press('F6');
        assert.deepEqual(await page.evaluate(()=>window.canvasKeys.splice(0)),['keydown','keyup'].map(type=>({type,key:'F6',code:'F6',location:0,repeat:false,ctrl:false,alt:false,shift:false,meta:false,target:'CANVAS'})),`${name}: retained choice must relay shortcut press/release to Winit`);
        await page.evaluate(()=>{
            window.keys=[];window.canvasKeys=[];
            const node={id:'1',key:'choice',role:'button',name:'Voir l’ouverture animée',hidden:false,disabled:false,next:'2',previous:'2'};
            window.bridge.render(JSON.stringify([node,{...node,id:'2',key:'other',name:'Rouvrir l’Atlas',next:'1',previous:'1'}]),'1','fr');
            window.bridge.events();
        });
        for(const key of ['F5','F6','F8','F9','F10','Escape']){
            await page.locator('[data-rvn-accessibility="1"]').focus();
            await page.evaluate(()=>window.bridge.events());
            await page.keyboard.press(key);
            const events=await page.evaluate(()=>window.keys.splice(0));
            assert.deepEqual(events,[{type:'keydown',key,prevented:true,target:'CANVAS'},{type:'keyup',key,prevented:true,target:'CANVAS'}],`${name}: ${key} must reach Winit without browser default or duplicate propagation`);
            assert.deepEqual(await page.evaluate(()=>window.canvasKeys.splice(0)),['keydown','keyup'].map(type=>({type,key,code:key,location:0,repeat:false,ctrl:false,alt:false,shift:false,meta:false,target:'CANVAS'})),`${name}: ${key} press/release must reach the actual canvas listener`);
        }
        await page.locator('[data-rvn-accessibility="1"]').focus();await page.evaluate(()=>window.bridge.events());
        const modifiers=await page.locator('[data-rvn-accessibility="1"]').evaluate(node=>{
            const event=new KeyboardEvent('keydown',{key:'F6',code:'F6',location:1,repeat:true,ctrlKey:true,altKey:true,shiftKey:true,metaKey:true,bubbles:true,cancelable:true});
            node.dispatchEvent(event);return event.defaultPrevented;
        });
        assert.ok(modifiers);
        assert.deepEqual(await page.evaluate(()=>window.canvasKeys.splice(0)),[{type:'keydown',key:'F6',code:'F6',location:1,repeat:true,ctrl:true,alt:true,shift:true,meta:true,target:'CANVAS'}],`${name}: relay must preserve key/code/location/repeat/modifiers`);
        await page.evaluate(()=>window.keys.splice(0));
        await page.locator('[data-rvn-accessibility="1"]').focus();await page.evaluate(()=>window.bridge.events());
        await page.keyboard.press('q');
        assert.deepEqual(await page.evaluate(()=>window.keys.splice(0)),[],`${name}: ordinary semantic control keys must stay local`);
        await page.keyboard.press('Tab');
        assert.deepEqual(await page.evaluate(()=>JSON.parse(window.bridge.events())),[{id:'2',action:'focus',value:null}],`${name}: authored Tab order must remain available`);
        const input=page.locator('input[data-rvn-element="atlas_first_name"]');
        await input.focus();await page.keyboard.press('End');await page.keyboard.type(' Q');
        assert.equal(await input.inputValue(),'Alix Q',`${name}: native RVN text editing must remain untouched`);
        await input.evaluate(node=>node.remove());
        await page.addScriptTag({content:inputAdapter+`\nwindow.inputs={render:rvn_render_inputs,events:rvn_input_events,focused:rvn_input_focused};`});
        await page.evaluate(font=>{
            window.field={screen:'atlas',element:'atlas_first_name',value:'Éloïse',placeholder:'',label:'Prénom',enabled:true,key_target:'atlas_root',layer:0,rect:[150,80,220,40],matrix:[1,0,0,1],clip:[[0,0],[220,0],[220,40],[0,40]],visible:true,font_size:16,padding:4,text_align:'left',color:'white'};
            window.inputs.render(JSON.stringify([window.field]),300,150,new Uint8Array(font),'atlas','atlas_first_name');
            window.inputs.events();window.keys=[];window.canvasKeys=[];
        },font);
        await input.fill('Éloïse Q');await input.press('ArrowLeft');
        const caret=await input.evaluate(node=>[node.selectionStart,node.selectionEnd]);
        await page.evaluate(()=>{window.field.value='Éloïse Q';window.inputs.render(JSON.stringify([window.field]),300,150,new Uint8Array(),'atlas','atlas_first_name');});
        assert.deepEqual(await input.evaluate(node=>[node.selectionStart,node.selectionEnd]),caret,`${name}: actual binding echo must retain caret`);
        await input.press('q');assert.equal(await input.inputValue(),'Éloïse qQ');
        assert.deepEqual(await page.evaluate(()=>window.canvasKeys.splice(0)),[],`${name}: actual native field editing may not reach the story`);
        await page.evaluate(()=>window.inputs.events());
        await input.press('Tab');
        assert.deepEqual(await page.evaluate(()=>JSON.parse(window.inputs.events())),[{screen:'atlas',element:'atlas_root',kind:'key',value:null,key:'Tab'},{screen:'atlas',element:'atlas_root',kind:'key',value:null,key:'__rvn_next_focus'}],`${name}: input Tab must retain the authored order`);
        await input.focus();await page.evaluate(()=>window.inputs.events());
        await input.press('F8');
        assert.deepEqual(await page.evaluate(()=>window.canvasKeys.splice(0)),['keydown','keyup'].map(type=>({type,key:'F8',code:'F8',location:0,repeat:false,ctrl:false,alt:false,shift:false,meta:false,target:'CANVAS'})),`${name}: actual native F8 press/release must reach Winit's Canvas`);
        assert.deepEqual(await page.evaluate(()=>JSON.parse(window.inputs.events())),[],`${name}: F8 may not be delivered as a narrative input event`);
        console.log(`${name} ${browser.version()}: PASS (rebuilt choice preserves focused DOM/current Entity action; 6 engine shortcuts to actual Canvas press/release, all modifier metadata; actual native F8, Unicode/q/caret/Tab preserved)`);
    }finally{await browser.close();}
}
