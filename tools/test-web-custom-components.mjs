// Real standalone exported-game QA, with software WebGL2 in Chrome and Firefox.
// node tools/test-web-custom-components.mjs EXPORT_DIRECTORY EVIDENCE_DIRECTORY
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve,join,extname} from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {homedir} from 'node:os';
import assert from 'node:assert/strict';
const [directory,output]=process.argv.slice(2);
if(!directory||!output)throw new Error('Expected exported game and evidence directories');
const root=resolve(directory),evidence=resolve(output);mkdirSync(evidence,{recursive:true});
const require=createRequire(import.meta.url),modules=process.env.RVN_QA_NODE_MODULES||join(homedir(),'.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules');
function dependency(name){try{return require(name);}catch{return require(join(modules,name));}}
const playwright=dependency('playwright'),{PNG}=dependency('pngjs');
const mime={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.png':'image/png','.ttf':'font/ttf','.toml':'text/plain','.rvn':'text/plain'};
const server=createServer((request,response)=>{try{
    const pathname=decodeURIComponent(new URL(request.url,'http://localhost').pathname),file=resolve(root,'.'+(pathname==='/'?'/index.html':pathname));
    if(!file.startsWith(root+'/')){response.writeHead(403);response.end();return;}
    const bytes=readFileSync(file);response.writeHead(200,{'Content-Type':mime[extname(file)]||'application/octet-stream'});response.end(bytes);
}catch{response.writeHead(404);response.end();}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url=`http://127.0.0.1:${server.address().port}/`,results={started_at:new Date().toISOString(),export_dir:root,files:{},browsers:[]};
for(const filename of ['game.js','game_bg.wasm','main.rvn','rvn.toml','theme.toml','assets/emblem.png']){const bytes=readFileSync(join(root,filename));results.files[filename]={bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};}
try{for(const name of ['chrome','firefox']){
    const result={name,checks:[],errors:[],warnings:[],not_found:[]};results.browsers.push(result);
    const browser=name==='chrome'?await playwright.chromium.launch({executablePath:process.env.RVN_CHROME_BIN||'/usr/bin/google-chrome',headless:true,args:['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader']}):await playwright.firefox.launch({headless:true,firefoxUserPrefs:{'webgl.force-enabled':true,'webgl.disabled':false,'gfx.webrender.all':true,'gfx.webrender.software':true}});
    result.version=browser.version();const context=await browser.newContext({viewport:{width:1280,height:720}});const page=await context.newPage();
    try{
        page.on('pageerror',error=>{if(String(error).includes('Using exceptions for control flow'))result.warnings.push(String(error));else result.errors.push(String(error));});
        page.on('console',message=>{if(message.type()==='error'){if(message.text().includes('404'))result.warnings.push(message.text());else result.errors.push(message.text());}else if(message.type()==='warning')result.warnings.push(message.text());});
        page.on('response',response=>{if(response.status()===404)result.not_found.push(new URL(response.url()).pathname);});
        await page.goto(url,{waitUntil:'domcontentloaded'});
        await page.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>node.textContent.includes('Two independent custom sliders')||node.textContent.includes('Deux curseurs personnalisés')),undefined,{timeout:60000});
        await page.waitForTimeout(1500);
        const bounds=await page.locator('canvas').boundingBox();
        const point=(x,y)=>({x:bounds.x+x*bounds.width/1920,y:bounds.y+y*bounds.height/1080});
        async function move(x,y){const p=point(x,y);await page.mouse.move(p.x,p.y);await page.waitForTimeout(120);}
        async function screenshot(label){const bytes=await page.screenshot({path:join(evidence,`${name}-${label}.png`)});return PNG.sync.read(bytes);}
        async function status(){return page.evaluate(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).map(node=>node.textContent).find(text=>/^(ready|pointer_[a-z]+|wheel|key|reset) \/ \d+/.test(text))||'');}
        async function eventStep(action,kind){
            const before=Number((await status()).split(' / ')[1]||0);await action();
            await page.waitForFunction(({before,kind})=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>node.textContent.startsWith(kind+' / ')&&Number(node.textContent.split(' / ')[1])>before),{before,kind},{timeout:12000});
            (result.pointer_events??=[]).push(await status());
        }
        const scalar=value=>value?.v??Object.values(value??{})[0];
        const savedLocal=(data,id)=>data.ui.screens.find(screen=>screen.name==='custom').canvas_states[id];
        const value=(data,id)=>scalar(scalar(savedLocal(data,id).state).value);
        async function save(){
            await page.evaluate(()=>document.querySelector('canvas').focus());
            await page.waitForTimeout(200);
            const previous=await page.evaluate(()=>Object.entries(localStorage).find(([key])=>key.includes('quicksave'))?.[1]);
            await page.keyboard.press('F5',{delay:150});
            await page.waitForFunction(previous=>Object.entries(localStorage).find(([key])=>key.includes('quicksave'))?.[1]!==previous,previous,{timeout:8000});
            return page.evaluate(()=>JSON.parse(Object.entries(localStorage).find(([key])=>key.includes('quicksave'))[1]));
        }
        const initialImage=await screenshot('initial');
        const pixel=(image,x,y)=>{const p=point(x,y),index=(Math.floor(p.y)*image.width+Math.floor(p.x))*4;return Array.from(image.data.subarray(index,index+4));};
        result.pixels={track:pixel(initialImage,220,355),canvas_background:pixel(initialImage,850,550),group_inner:pixel(initialImage,195,485),group_outside:pixel(initialImage,160,540),image:pixel(initialImage,317,500)};
        assert.ok(result.pixels.track[1]>100&&result.pixels.track[0]<80,'Authored turquoise primitive must be visible');
        assert.ok(result.pixels.group_inner[2]>result.pixels.canvas_background[2]+15,'Transformed clipped group must be visible');
        assert.ok(Math.abs(result.pixels.group_outside[2]-result.pixels.canvas_background[2])<15,'Group clip must exclude outside geometry');
        let brightGlyphs=0;for(let y=285;y<325;y++)for(let x=180;x<300;x++){const color=pixel(initialImage,x,y);if(color[0]>150&&color[1]>150&&color[2]>150)brightGlyphs++;}
        assert.ok(brightGlyphs>15,'Canvas text must have real visible glyphs');result.glyph_samples=brightGlyphs;
        result.checks.push('Standalone export renders shapes, inherited-font text and transformed clipped group');
        const initial=await save();result.initial=initial;assert.equal(initial.format_version,9);assert.equal(value(initial,'first'),0.25);assert.equal(value(initial,'second'),0.75);
        assert.ok(savedLocal(initial,'first').elapsed>0&&scalar(scalar(savedLocal(initial,'first').state).updates)>0);
        result.initial=initial;result.checks.push('Independent authored initial states and simulation ticks are serialized in save format 9');
        await move(350,358);await eventStep(()=>page.mouse.down(),'pointer_down');
        await eventStep(()=>move(820,358),'pointer_move');
        await eventStep(()=>move(940,358),'pointer_move');
        await eventStep(()=>page.mouse.up(),'pointer_up');await page.waitForTimeout(400);
        const dragged=await save();result.dragged=dragged;assert.equal(value(dragged,'first'),1);assert.equal(value(dragged,'second'),0.75);assert.equal(scalar(scalar(savedLocal(dragged,'first').state).dragging),false);assert.equal(scalar(dragged.vars.custom_last_event),'pointer_up');
        result.dragged=dragged;result.checks.push('Real pointer down/move/up outside Canvas retains capture and changes only the targeted instance');
        await move(400,358);await page.mouse.wheel(0,120);await page.waitForTimeout(300);
        const wheel=await save();assert.notEqual(value(wheel,'first'),1);assert.notEqual(scalar(wheel.vars.custom_last_wheel),0);
        result.wheel=wheel;result.checks.push('Real wheel input supplies signed delta and changes local state');
        await move(400,358);await page.mouse.click(point(400,358).x,point(400,358).y);await page.waitForTimeout(150);
        await page.keyboard.down('ArrowLeft');await page.waitForTimeout(150);await page.keyboard.up('ArrowLeft');await page.waitForTimeout(150);
        const keyed=await save();assert.equal(scalar(keyed.vars.custom_last_key),'ArrowLeft');assert.ok(scalar(keyed.vars.custom_event_count)>=scalar(wheel.vars.custom_event_count)+4);
        result.keyed=keyed;result.checks.push('Keyboard press and release reach Canvas handler with code and independent local state');
        await screenshot('edited');
        await move(400,358);await page.mouse.click(point(400,358).x,point(400,358).y);await page.waitForTimeout(150);await page.keyboard.press('End');await page.waitForTimeout(200);
        await page.evaluate(()=>document.querySelector('canvas').focus());await page.keyboard.press('F6',{delay:150});await page.waitForTimeout(500);await screenshot('load-confirmation');
        await page.keyboard.press('ArrowLeft');await page.waitForTimeout(200);await page.keyboard.press('Enter',{delay:150});await page.waitForTimeout(600);
        const restored=await save();assert.ok(Math.abs(value(restored,'first')-value(keyed,'first'))<0.001);assert.equal(value(restored,'second'),0.75);
        result.restored=restored;result.checks.push('Approved quickload reconstructs drawings and restores both instance states without stale capture');
        await screenshot('restored');
        const input=page.locator('input[data-rvn-screen="custom"][data-rvn-element="note"]');
        await input.waitFor({state:'visible'});assert.equal(await input.inputValue(),'Camille');
        const original=await input.elementHandle();await input.click();await input.fill('Éloïse 😀');await page.waitForTimeout(300);
        assert.equal(await input.inputValue(),'Éloïse 😀');assert.ok(await original.evaluate(node=>node===document.activeElement&&node===document.querySelector('[data-rvn-element="note"]')));
        await input.press('ArrowLeft');const caret=await input.evaluate(node=>node.selectionStart);assert.ok(caret>0&&caret<nodeTextLength('Éloïse 😀'));
        await input.press('Shift+Tab');await page.waitForTimeout(300);
        await page.waitForFunction(()=>document.activeElement===document.querySelector('canvas'));
        await page.keyboard.press('ArrowRight',{delay:150});await page.waitForTimeout(300);
        const afterInput=await save();assert.equal(scalar(afterInput.vars.custom_note),'Éloïse 😀');assert.ok(value(afterInput,'second')>0.75);
        result.after_input=afterInput;result.input_caret=caret;result.checks.push('Unicode native input keeps identity/caret across redraw; Shift+Tab returns focus and key routing to Canvas');
        assert.deepEqual(result.errors,[],'No runtime/page error is allowed');assert.ok(!result.not_found.includes('/assets/emblem.png'),'Required canvas image must load');
        result.checks.push('Exported original image loads and runtime emits no fatal diagnostic');
        const missing=await context.newPage();await missing.route('**/assets/emblem.png',route=>route.fulfill({status:404,body:'Missing owned QA image'}));
        await missing.goto(url,{waitUntil:'domcontentloaded'});await missing.waitForTimeout(2500);
        const failedImage=PNG.sync.read(await missing.screenshot({path:join(evidence,`${name}-missing-image.png`)}));
        let red=0;for(let i=0;i<failedImage.data.length;i+=4){if(failedImage.data[i]>120&&failedImage.data[i+1]<90&&failedImage.data[i+2]<90)red++;}
        assert.ok(red>500,'Missing drawing resource must produce a visible error card, not a silent empty image');
        result.missing_image={red_error_pixels:red};await missing.close();result.checks.push('Missing Canvas image produces a visible runtime diagnostic');
        result.status='passed';console.log(`${name} ${result.version}: PASS (${result.checks.length} checks)`);
    }catch(error){result.status='failed';result.failure=String(error);console.error(`${name}: ${error}`);result.semantic=await page.evaluate(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).map(node=>({text:node.textContent,label:node.getAttribute('aria-label')}))).catch(()=>null);await page.screenshot({path:join(evidence,`${name}-failure.png`)}).catch(()=>{});}
    finally{await context.close();await browser.close();}
}}finally{server.close();results.finished_at=new Date().toISOString();writeFileSync(join(evidence,'web-custom-components-evidence.json'),JSON.stringify(results,null,2)+'\n');}
if(results.browsers.some(result=>result.status!=='passed'))process.exitCode=1;
function nodeTextLength(text){return text.length;}
