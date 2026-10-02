// Actual exported-game integration QA; not a mocked renderer or editor.
// Usage: node tools/test-web-interface-export.mjs EXPORT_DIR EVIDENCE_DIR
// Requires Playwright, a system Chrome and Playwright's Firefox runtime.
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve,join,extname} from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {homedir} from 'node:os';
import assert from 'node:assert/strict';

const [directory,output]=process.argv.slice(2);
if(!directory||!output)throw new Error('Expected exported fixture directory and evidence directory');
const root=resolve(directory),evidence=resolve(output);mkdirSync(evidence,{recursive:true});
const require=createRequire(import.meta.url);
const fallbackModules=process.env.RVN_QA_NODE_MODULES||join(homedir(),'.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules');
function dependency(name){
    try{return require(name);}
    catch{return require(join(fallbackModules,name));}
}
const playwright=dependency('playwright'),{PNG}=dependency('pngjs');
const mime={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.ttf':'font/ttf','.otf':'font/otf','.toml':'text/plain','.rvn':'text/plain'};
const server=createServer((request,response)=>{
    try {
        const pathname=decodeURIComponent(new URL(request.url,'http://localhost').pathname);
        const file=resolve(root,'.'+(pathname==='/'?'/index.html':pathname));
        if(!file.startsWith(root+'/')){response.writeHead(403);response.end();return;}
        const bytes=readFileSync(file);response.writeHead(200,{'Content-Type':mime[extname(file)]||'application/octet-stream'});response.end(bytes);
    }catch{response.writeHead(404);response.end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url=`http://127.0.0.1:${server.address().port}/`;
const results={started_at:new Date().toISOString(),export_dir:root,files:{},browsers:[]};
for(const filename of ['game.js','game_bg.wasm','main.rvn','rvn.toml','theme.toml','assets/fonts/DejaVuSerif.ttf']){
    const bytes=readFileSync(join(root,filename));results.files[filename]={bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};
}
try {
    for(const name of ['chrome','firefox']){
        const result={name,checks:[],errors:[],warnings:[],not_found:[]};results.browsers.push(result);
        const browser=name==='chrome'
            ?await playwright.chromium.launch({executablePath:process.env.RVN_CHROME_BIN||'/usr/bin/google-chrome',headless:true,args:['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader']})
            :await playwright.firefox.launch({headless:true,firefoxUserPrefs:{'webgl.force-enabled':true,'webgl.disabled':false,'gfx.webrender.all':true,'gfx.webrender.software':true}});
        result.version=browser.version();
        const context=await browser.newContext({viewport:{width:1280,height:720}});
        try {
            const page=await context.newPage();
            await page.addInitScript(()=>{globalThis.__rvnShaders=[];const base=WebGL2RenderingContext.prototype.shaderSource;WebGL2RenderingContext.prototype.shaderSource=function(shader,source){globalThis.__rvnShaders.push(source);return base.call(this,shader,source);};});
            page.on('pageerror',error=>{if(String(error).includes('Using exceptions for control flow')){result.warnings.push(String(error));}else result.errors.push(String(error));});
            page.on('response',response=>{if(response.status()===404)result.not_found.push(new URL(response.url()).pathname);});
            page.on('console',message=>{if(message.type()==='error'){if(message.text().includes('Failed to load resource: the server responded with a status of 404'))result.warnings.push(message.text());else result.errors.push(message.text());}else if(message.type()==='warning')result.warnings.push(message.text());});
            await page.goto(url,{waitUntil:'domcontentloaded'});
            const input=page.locator('input[data-rvn-screen="qa"][data-rvn-element="name"]');
            await input.waitFor({state:'visible',timeout:45000});
            const selected=page.locator('input[data-rvn-screen="qa"][data-rvn-element="result"]');
            await page.waitForFunction(()=>document.querySelector('[data-rvn-element="result"]')?.value==='none');
            assert.equal(await input.inputValue(),'Camille');
            result.checks.push('Export starts independently of editor, bindings initialized');
            await page.waitForFunction(()=>Array.from(document.fonts).some(face=>face.family.replaceAll('"','')==='RVN Interface 1'&&face.status==='loaded'),undefined,{timeout:5000});
            const inputStyle=await input.evaluate(node=>({font:getComputedStyle(node).fontFamily,align:getComputedStyle(node).textAlign,color:getComputedStyle(node).color}));
            assert.ok(inputStyle.font.includes('RVN Interface 1'));assert.equal(inputStyle.align,'center');
            result.input_style=inputStyle;result.checks.push('Inherited project font loaded in canvas and DOM, centered native input');
            const initial=PNG.sync.read(await page.screenshot({path:join(evidence,`${name}-initial.png`)}));
            const pixel=(x,y)=>Array.from(initial.data.subarray((y*initial.width+x)*4,(y*initial.width+x)*4+4));
            result.background_pixel=pixel(1000,650);assert.ok(result.background_pixel[0]<35&&result.background_pixel[1]<45&&result.background_pixel[2]<55,'Border material must not fill the background');
            result.button_corner=pixel(54,108);result.button_inner=pixel(180,130);result.button_border=pixel(53,130);
            assert.ok(result.button_corner[0]<35&&result.button_corner[1]<45,'Rounded corners must expose the parent, not draw a square');
            assert.ok(result.button_border[0]>150&&result.button_border[1]>100&&result.button_inner[0]<35,'Focus border and interior must remain distinct materials');
            result.checks.push('Real rounded background/border/focus materials retain authored colors');
            const canvas=await page.locator('canvas').boundingBox();
            const small=page.locator('input[data-rvn-screen="qa"][data-rvn-element="small"]');
            const smallFont=await small.evaluate(node=>parseFloat(getComputedStyle(node).fontSize));
            assert.ok(Math.abs(smallFont-6*canvas.height/1080)<0.01,'Authored small typography must retain its exact reference scale');
            // The caption is rendered by Bevy, not a DOM input. Its actual
            // bright glyph bounds independently catch an old 14 px clamp.
            const tinyRows=[];
            for(let y=Math.floor(canvas.y+570*canvas.height/1080);y<Math.ceil(canvas.y+590*canvas.height/1080);y++){
                for(let x=Math.floor(canvas.x+80*canvas.width/1920);x<Math.ceil(canvas.x+680*canvas.width/1920);x++){
                    const offset=(y*initial.width+x)*4;
                    if(initial.data[offset]>100&&initial.data[offset+1]>100&&initial.data[offset+2]>100){tinyRows.push(y);break;}
                }
            }
            assert.ok(tinyRows.length>0&&tinyRows.at(-1)-tinyRows[0]+1<=7,'Authored tiny canvas text must be visible and scale without a readability floor');
            result.small_typography={reference_size:6,dom_css_px:smallFont,canvas_glyph_rows:tinyRows,canvas_glyph_height:tinyRows.at(-1)-tinyRows[0]+1};
            result.checks.push('Authored 6 px text scales to 4 CSS px in native input and visible tiny canvas glyphs');
            const bounds=await input.boundingBox();
            assert.ok(Math.abs(bounds.x-(canvas.x+80*canvas.width/1920))<1&&Math.abs(bounds.y-(canvas.y+260*canvas.height/1080))<1,'Child positions use the parent outer border, matching the designer');
            result.checks.push('Nested authored rectangles match shared reference geometry despite parent borders');
            await page.mouse.move(canvas.x+280*canvas.width/1920,canvas.y+195*canvas.height/1080);
            await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
            // Allow the first software-WebGL shader warm-up to finish before
            // a press/release pair, rather than delivering both within a frame.
            await page.mouse.down();await page.waitForTimeout(300);await page.mouse.up();
            await page.waitForFunction(()=>document.querySelector('[data-rvn-element="result"]')?.value==='ruby');
            result.checks.push('Reusable styled button click receives event_data and updates binding');
            const original=await input.elementHandle();
            await input.fill('Éloïse');
            await page.waitForFunction(()=>document.querySelector('[data-rvn-element="name"]')?.value==='Éloïse');
            await page.waitForFunction(()=>document.querySelector('[data-rvn-element="result"]')?.value==='ruby');
            assert.ok(await original.evaluate(node=>node===document.querySelector('[data-rvn-element="name"]')));
            result.checks.push('Unicode editing preserves native input identity across reactive refresh');
            await input.press('Tab');
            await page.waitForFunction(()=>document.activeElement?.getAttribute('data-rvn-element')==='result');
            result.checks.push('Authored keyboard focus order traverses inputs without trap');
            await page.evaluate(()=>{document.activeElement?.blur();document.querySelector('canvas').focus();});
            await page.keyboard.press('F5');
            await page.waitForFunction(()=>Object.keys(localStorage).some(key=>key.includes('quicksave')));
            result.saved=await page.evaluate(()=>Object.fromEntries(Object.entries(localStorage).filter(([key])=>key.includes('quicksave'))));
            const data=JSON.parse(Object.values(result.saved)[0]);
            assert.equal(data.vars.player_name.v,'Éloïse');assert.equal(data.vars.selected.v,'ruby');
            result.checks.push('Browser quicksave serializes current game variables and screen state');
            await page.screenshot({path:join(evidence,`${name}-edited.png`)});
            await input.fill('Changed after save');
            await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
            await page.evaluate(()=>{document.activeElement?.blur();document.querySelector('canvas').focus();});
            await page.keyboard.press('F6');
            await input.waitFor({state:'hidden'});
            await page.screenshot({path:join(evidence,`${name}-load-confirmation.png`)});
            // The safe default is Cancel. Select Confirm explicitly in the
            // authored dialog, rather than bypassing its approval gate.
            await page.keyboard.press('ArrowLeft');await page.waitForTimeout(200);
            await page.keyboard.press('Enter',{delay:200});
            await input.waitFor({state:'visible'});
            await page.waitForFunction(()=>document.querySelector('[data-rvn-element="name"]')?.value==='Éloïse'&&document.querySelector('[data-rvn-element="result"]')?.value==='ruby');
            assert.ok((await input.evaluate(node=>getComputedStyle(node).fontFamily)).includes('RVN Interface 1'));
            result.checks.push('Approved quickload restores variables, open interface, inherited font and live bindings');
            // The input bridge updates before Bevy's glyph/layout render pass.
            // Capture a completed frame, not that intermediate UI rebuild.
            await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
            await page.screenshot({path:join(evidence,`${name}-restored.png`)});
            assert.deepEqual(result.not_found.filter(path=>!['/config.toml','/menus.toml','/menus.rvnui','/locales/fr.toml','/favicon.ico','/config.toml.meta','/assets/fonts/DejaVuSerif.ttf.meta'].includes(path)),[],'Required exported resources must not return 404');
            assert.deepEqual(result.errors,[],'Export must not emit page or runtime errors');
            const missing=await context.newPage();const resourceErrors=[];
            missing.on('console',message=>{if(message.type()==='error')resourceErrors.push(message.text());});
            await missing.route('**/assets/fonts/DejaVuSerif.ttf',route=>route.fulfill({status:404,body:'Missing owned QA font'}));
            const failedFont=missing.waitForResponse(response=>response.url().endsWith('/assets/fonts/DejaVuSerif.ttf')&&response.status()===404);
            await missing.goto(url,{waitUntil:'domcontentloaded'});await failedFont;
            await missing.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>node.textContent.includes('interface resource could not be loaded')||node.textContent.includes('Interface font could not be loaded')),undefined,{timeout:5000}).catch(()=>{});
            // Error text may be a canvas-only label, but the actual error card
            // must be visible and native editable controls must be removed.
            await missing.waitForTimeout(500);
            const failureImage=PNG.sync.read(await missing.screenshot({path:join(evidence,`${name}-missing-font.png`)}));
            let red=0;for(let i=0;i<failureImage.data.length;i+=4){if(failureImage.data[i]>120&&failureImage.data[i+1]<80&&failureImage.data[i+2]<80)red++;}
            assert.ok(red>500,'Missing fonts must produce a visible error card, not a silent fallback');
            assert.equal(await missing.locator('input[data-rvn-screen="qa"]').count(),0);
            result.missing_font={red_error_pixels:red,console:resourceErrors};
            result.checks.push('Missing project font produces visible runtime diagnostic and disables the invalid interface');
            await missing.close();
            result.status='passed';
            console.log(`${name} ${result.version}: PASS (${result.checks.length} integration checks)`);
        }catch(error){
            result.status='failed';result.failure=String(error);console.error(`${name}: ${error}`);
            result.diagnostics=await context.pages()[0]?.evaluate(()=>({fonts:Array.from(document.fonts).map(face=>({family:face.family,status:face.status})),inputs:Array.from(document.querySelectorAll('input')).map(node=>({id:node.getAttribute('data-rvn-element'),value:node.value,style:node.style.cssText})),texts:Array.from(document.querySelectorAll('[data-rvn-accessibility]')).map(node=>node.textContent)})).catch(()=>null);
            const shaders=await context.pages()[0]?.evaluate(()=>globalThis.__rvnShaders||[]).catch(()=>[]);writeFileSync(join(evidence,`${name}-shaders.json`),JSON.stringify(shaders,null,2));
            await context.pages()[0]?.screenshot({path:join(evidence,`${name}-failure.png`)}).catch(()=>{});
        }finally{await context.close();await browser.close();}
    }
}finally{
    server.close();results.finished_at=new Date().toISOString();writeFileSync(join(evidence,'web-interface-evidence.json'),JSON.stringify(results,null,2)+'\n');
}
if(results.browsers.some(result=>result.status!=='passed'))process.exitCode=1;
