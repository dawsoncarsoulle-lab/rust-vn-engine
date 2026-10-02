// Focused real-browser regression: down/up are sent in one native input burst.
// No synthetic DOM events, engine calls, seek, user profile or user window.
import {createServer} from 'node:http';
import {readFileSync,existsSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve,join,extname} from 'node:path';
import {createRequire} from 'node:module';
import {homedir} from 'node:os';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const [directory,output]=process.argv.slice(2);
assert.ok(directory&&output,'Expected export and fresh evidence directories');
const root=resolve(directory),evidence=resolve(output);
assert.ok(!existsSync(evidence),'Evidence directory must be fresh');mkdirSync(evidence,{recursive:true});
const require=createRequire(import.meta.url);
const modules=process.env.RVN_QA_NODE_MODULES||join(homedir(),'.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules');
let playwright;try{playwright=require('playwright');}catch{playwright=require(join(modules,'playwright'));}
const results={started_at:new Date().toISOString(),export_dir:root,pointer_requested_hold_ms:0,method:'Playwright native pointer click requests delay:0; browser-observed trusted timestamps, not requested delay, determine the short-click assertion. No synthetic DOM events or engine calls.',files:{},browsers:[]};
for(const file of ['main.rvn','game_bg.wasm']){const bytes=readFileSync(join(root,file));results.files[file]={bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};}
const mime={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.png':'image/png','.jpg':'image/jpeg','.ttf':'font/ttf','.otf':'font/otf','.webm':'video/webm','.wav':'audio/wav','.rvn':'text/plain','.toml':'text/plain'};
const server=createServer((request,response)=>{
    try{const pathname=decodeURIComponent(new URL(request.url,'http://localhost').pathname);
        if(pathname==='/favicon.ico'){response.writeHead(204);response.end();return;}
        const path=resolve(root,'.'+(pathname==='/'?'/index.html':pathname));
        if(!path.startsWith(root+'/')){response.writeHead(403);response.end();return;}
        const bytes=readFileSync(path);response.writeHead(200,{'Content-Type':mime[extname(path)]||'application/octet-stream'});response.end(bytes);
    }catch{response.writeHead(404);response.end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
results.server_port=server.address().port;
const url=`http://127.0.0.1:${results.server_port}/`;
try{
    for(const name of ['chrome','firefox']){
        const result={name,checks:[],errors:[],warnings:[],not_found:[]};results.browsers.push(result);
        const browser=name==='chrome'
            ?await playwright.chromium.launch({executablePath:process.env.RVN_CHROME_BIN||'/usr/bin/google-chrome',headless:true,args:['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader']})
            :await playwright.firefox.launch({headless:true,firefoxUserPrefs:{'webgl.force-enabled':true,'webgl.disabled':false,'gfx.webrender.all':true,'gfx.webrender.software':true}});
        result.version=browser.version();
        const context=await browser.newContext({viewport:{width:1280,height:720}}),page=await context.newPage();
        try{
            page.on('pageerror',error=>String(error).includes('Using exceptions for control flow')?result.warnings.push(String(error)):result.errors.push(String(error)));
            page.on('console',message=>{if(message.type()==='error'&&!message.text().includes('404'))result.errors.push(message.text());});
            page.on('response',response=>{if(response.status()===404)result.not_found.push(new URL(response.url()).pathname);});
            await page.goto(url,{waitUntil:'domcontentloaded'});
            const atlas=()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>!node.hidden&&node.textContent.includes('Échap · Retour'));
            await page.waitForFunction(atlas,undefined,{timeout:60000});await page.waitForTimeout(1200);
            await page.evaluate(()=>{
                window.atlasShortPointers=[];
                for(const type of ['pointerdown','pointerup'])document.querySelector('canvas').addEventListener(type,event=>{
                    window.atlasShortPointers.push({type,at:event.timeStamp,observed_at:performance.now(),pointer:event.pointerId,button:event.button,trusted:event.isTrusted});
                },true);
            });
            async function click(x,y){const canvas=page.locator('canvas'),bounds=await canvas.boundingBox();await canvas.focus();
                await page.mouse.move(bounds.x+x*bounds.width/1920,bounds.y+y*bounds.height/1080);await page.waitForTimeout(250);
                await page.mouse.click(bounds.x+x*bounds.width/1920,bounds.y+y*bounds.height/1080,{delay:0});}
            async function save(){await page.waitForFunction(()=>!Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>!node.hidden&&node.textContent.includes('Échap · Retour')));
                // Atlas disappearing precedes the motion fence reaching Waiting;
                // match full QA's post-pointer settle before testing an unrelated save.
                await page.waitForTimeout(350);
                const previous=await page.evaluate(()=>Object.entries(localStorage).find(([key])=>key.includes('quicksave'))?.[1]);
                // Keep keyboard synchronization separate from the pointer regression,
                // matching the successful full browser QA rather than a short F5 test.
                await page.locator('canvas').focus();await page.keyboard.press('F5',{delay:100});
                await page.waitForFunction(previous=>Object.entries(localStorage).find(([key])=>key.includes('quicksave'))?.[1]!==previous,previous,{timeout:15000});
                return page.evaluate(()=>JSON.parse(Object.entries(localStorage).find(([key])=>key.includes('quicksave'))[1]));}
            async function reopen(){await page.getByRole('button',{name:/Rouvrir l’Atlas/}).waitFor({state:'visible'});
                await page.locator('canvas').focus();await page.keyboard.press('2',{delay:30});await page.waitForFunction(atlas);}
            const field=page.locator('input[data-rvn-screen="atlas"][data-rvn-element="atlas_first_name"]');
            await click(565,134);await field.waitFor({state:'visible',timeout:15000});assert.equal(await field.inputValue(),'Alix');
            await page.screenshot({path:join(evidence,`${name}-01-short-identity.png`)});
            result.checks.push('A trusted native down/up burst activates the actual identity tab and creates its bound input');
            await click(960,134);await field.waitFor({state:'detached',timeout:15000});await click(1685,1014);
            const inventory=await save();assert.equal(inventory.vars.atlas_page?.v??inventory.vars.atlas_page?.Int,1);
            result.checks.push('Short actual inventory and return buttons close Atlas; actual F5 records atlas_page=1');
            await reopen();await click(1355,134);await page.waitForTimeout(350);
            await page.screenshot({path:join(evidence,`${name}-02-short-map.png`)});
            await click(1685,1014);const map=await save();assert.equal(map.vars.atlas_page?.v??map.vars.atlas_page?.Int,2);
            result.checks.push('Short actual map and return buttons close Atlas; actual F5 records atlas_page=2');
            result.pointer_evidence=await page.evaluate(()=>window.atlasShortPointers);
            const pending=new Map();result.pointer_observed_hold_ms=[];
            for(const event of result.pointer_evidence){if(event.type==='pointerdown')pending.set(event.pointer,event);else if(pending.has(event.pointer)){
                result.pointer_observed_hold_ms.push(event.at-pending.get(event.pointer).at);pending.delete(event.pointer);}}
            assert.equal(result.pointer_observed_hold_ms.length,5);assert.equal(pending.size,0);
            assert.ok(result.pointer_evidence.every(event=>event.trusted));
            assert.ok(result.pointer_observed_hold_ms.every(duration=>duration>=0&&duration<50),'Every observed native down/up interval must be below 50 ms');
            result.required_not_found=[...new Set(result.not_found.filter(path=>path!=='/menus.rvnui'&&!path.endsWith('.meta')))];
            assert.deepEqual(result.required_not_found,[]);assert.deepEqual(result.errors,[]);
            result.checks.push('All five browser-observed native pointer intervals are below 50 ms and trusted; no required asset or fatal error');
            result.status='passed';console.log(`${name} ${result.version}: PASS (${result.checks.length} checks; intervals ${result.pointer_observed_hold_ms.join(', ')} ms)`);
        }catch(error){result.status='failed';result.failure=String(error);console.error(`${name}: ${error}`);
            result.pointer_evidence=await page.evaluate(()=>window.atlasShortPointers??[]).catch(()=>[]);
            await page.screenshot({path:join(evidence,`${name}-failure.png`)}).catch(()=>{});
        }finally{await context.close();await browser.close();}
    }
}finally{
    await new Promise(done=>server.close(done));results.finished_at=new Date().toISOString();
    writeFileSync(join(evidence,'pointer-evidence.json'),JSON.stringify(results,null,2)+'\n');
}
if(results.browsers.some(result=>result.status!=='passed'))process.exitCode=1;
