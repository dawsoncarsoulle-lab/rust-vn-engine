// Real standalone export, real pointer/keyboard/DOM inputs; no engine test hooks.
// node examples/atlas-des-brumes/qa/web.mjs EXPORT_DIRECTORY EVIDENCE_DIRECTORY
import {createServer} from 'node:http';
import {readFileSync, readdirSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve, join, extname} from 'node:path';
import {createRequire} from 'node:module';
import {homedir} from 'node:os';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';

const [directory, output] = process.argv.slice(2);
if (!directory || !output) throw new Error('Expected export directory and evidence directory');
const root = resolve(directory), evidence = resolve(output);
mkdirSync(evidence, {recursive:true});
const pointerHold = Number(process.env.ATLAS_QA_POINTER_HOLD || 30);
assert.ok(Number.isFinite(pointerHold)&&pointerHold>=0,'Pointer hold must be a finite nonnegative duration');
const require = createRequire(import.meta.url);
const modules = process.env.RVN_QA_NODE_MODULES || join(homedir(), '.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules');
function dependency(name) { try { return require(name); } catch { return require(join(modules,name)); } }
const playwright = dependency('playwright'), {PNG} = dependency('pngjs');
const mime = {'.html':'text/html', '.js':'text/javascript', '.wasm':'application/wasm', '.png':'image/png', '.jpg':'image/jpeg', '.ttf':'font/ttf', '.otf':'font/otf', '.webm':'video/webm', '.wav':'audio/wav', '.rvn':'text/plain', '.toml':'text/plain'};
const server = createServer((request,response) => {
    try {
        const pathname = decodeURIComponent(new URL(request.url,'http://localhost').pathname);
        if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
        const file = resolve(root, '.'+(pathname==='/'?'/index.html':pathname));
        if (!file.startsWith(root+'/')) { response.writeHead(403); response.end(); return; }
        const bytes = readFileSync(file);
        response.writeHead(200, {'Content-Type':mime[extname(file)] || 'application/octet-stream'}); response.end(bytes);
    } catch { response.writeHead(404); response.end(); }
});
await new Promise(done => server.listen(0,'127.0.0.1',done));
const url = `http://127.0.0.1:${server.address().port}/`;
const results = {started_at:new Date().toISOString(), export_dir:root, pointer_hold_ms:pointerHold, pointer_hover_settle_ms:250, pointer_post_input_settle_ms:350, files:{}, browsers:[]};
function recordFiles(directory,relative='') {
    for (const entry of readdirSync(directory,{withFileTypes:true})) {
        const file = join(relative,entry.name);
        if (entry.isDirectory()) recordFiles(join(directory,entry.name),file);
        else { const bytes=readFileSync(join(directory,entry.name)); results.files[file]={bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')}; }
    }
}
recordFiles(root);
const source=readFileSync(join(root,'main.rvn'),'utf8');
const places=[...source.matchAll(/\{"id":"(havre|observatoire|clairiere|archives|falaises|tour)","name":"[^"]+","short":"[^"]+","x":([\d.]+),"y":([\d.]+)/g)].map(match=>({id:match[1],x:Number(match[2]),y:Number(match[3])}));
assert.equal(places.length,6,'The exported canonical source must declare the six actual map markers');

try {
    for (const name of (process.env.ATLAS_QA_BROWSER ? [process.env.ATLAS_QA_BROWSER] : ['chrome','firefox'])) {
        const result={name,checks:[],errors:[],warnings:[],not_found:[]}; results.browsers.push(result);
        const browser = name==='chrome'
            ? await playwright.chromium.launch({executablePath:process.env.RVN_CHROME_BIN || '/usr/bin/google-chrome',headless:true,args:['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader']})
            : await playwright.firefox.launch({headless:true,firefoxUserPrefs:{'webgl.force-enabled':true,'webgl.disabled':false,'gfx.webrender.all':true,'gfx.webrender.software':true}});
        result.version=browser.version();
        const context=await browser.newContext({viewport:{width:1280,height:720}});
        const page=await context.newPage();
        try {
            page.on('pageerror',error=>{if(String(error).includes('Using exceptions for control flow'))result.warnings.push(String(error));else result.errors.push(String(error));});
            page.on('console',message=>{if(message.type()==='error'){if(message.text().includes('404'))result.warnings.push(message.text());else result.errors.push(message.text());}else if(message.type()==='warning')result.warnings.push(message.text());});
            page.on('response',response=>{if(response.status()===404)result.not_found.push(new URL(response.url()).pathname);});
            await page.goto(url,{waitUntil:'domcontentloaded'});
            async function waitAtlas() {
                await page.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>!node.hidden&&node.textContent.includes('Échap · Retour')),undefined,{timeout:60000});
            }
            async function waitNoAtlas() {
                await page.waitForFunction(()=>!Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>!node.hidden&&node.textContent.includes('Échap · Retour')),undefined,{timeout:15000});
            }
            async function point(x,y) {
                const bounds=await page.locator('canvas').boundingBox();
                return {x:bounds.x+x*bounds.width/1920,y:bounds.y+y*bounds.height/1080};
            }
            async function mouse(x,y) {
                const p=await point(x,y); await page.locator('canvas').focus();
                // Let Winit process the actual hover position before pressing;
                // software WebGL resize frames must not coalesce move+press.
                await page.mouse.move(p.x,p.y); await page.waitForTimeout(250);
                await page.mouse.click(p.x,p.y,{delay:pointerHold});
                await page.waitForTimeout(350);
            }
            async function move(x,y) { const p=await point(x,y); await page.mouse.move(p.x,p.y); await page.waitForTimeout(350); }
            async function key(code) { await page.locator('canvas').focus(); await page.keyboard.press(code,{delay:100}); await page.waitForTimeout(350); }
            async function screenshot(label) { return PNG.sync.read(await page.screenshot({path:join(evidence,`${name}-${label}.png`)})); }
            async function videoScreenshot(label,start,end) {
                // Screenshot synchronization itself can consume this 6 s clip
                // under SwiftShader. Observe the real, already rendered Bevy
                // canvas in a frame callback instead; never seek or pause it.
                const capture=await page.evaluate(({start,end})=>new Promise((resolve,reject)=>{
                    const canvas=document.querySelector('canvas'),copy=document.createElement('canvas');
                    copy.width=canvas.width;copy.height=canvas.height;
                    const context=copy.getContext('2d',{willReadFrequently:true}),deadline=performance.now()+15000;
                    const frame=()=>{
                        const video=document.querySelector('video');
                        if(performance.now()>deadline)return reject(new Error('Timed out observing real rendered subtitle pixels'));
                        if(video?.currentTime>=end)return reject(new Error(`Subtitle capture window missed at ${video.currentTime}`));
                        if(video&&video.readyState>=2&&!video.paused&&video.currentTime>=start){
                            context.drawImage(canvas,0,0);
                            const pixels=context.getImageData(120,650,1040,55).data;let white=0;
                            for(let i=0;i<pixels.length;i+=4)if(pixels[i]>215&&pixels[i+1]>215&&pixels[i+2]>215&&pixels[i+3]>0)white++;
                            if(white>120)return resolve({png:copy.toDataURL('image/png'),white,time:video.currentTime,width:video.videoWidth,height:video.videoHeight,duration:video.duration,readyState:video.readyState,paused:video.paused});
                        }
                        requestAnimationFrame(frame);
                    };
                    requestAnimationFrame(frame);
                }),{start,end});
                const bytes=Buffer.from(capture.png.split(',')[1],'base64');delete capture.png;
                writeFileSync(join(evidence,`${name}-${label}.png`),bytes);
                return {png:PNG.sync.read(bytes),observed:capture};
            }
            const scalar=value=>value?.v??Object.values(value??{})[0];
            const variable=(save,id)=>scalar(save.vars[id]);
            async function quicksave() {
                await waitNoAtlas();
                const previous=await page.evaluate(()=>Object.entries(localStorage).find(([key])=>key.includes('quicksave'))?.[1]);
                await key('F5');
                await page.waitForFunction(previous=>Object.entries(localStorage).find(([key])=>key.includes('quicksave'))?.[1]!==previous,previous,{timeout:10000});
                return page.evaluate(()=>JSON.parse(Object.entries(localStorage).find(([key])=>key.includes('quicksave'))[1]));
            }
            async function identity() { await mouse(565,134); await page.locator('input[data-rvn-screen="atlas"][data-rvn-element="atlas_first_name"]').waitFor({state:'visible'}); }
            async function tab(pageIndex) { await mouse([565,960,1355][pageIndex],134); await page.waitForTimeout(450); }
            async function reopenAfterForest() {
                // The current real dialogue is the locked-first-visit forest warning.
                await key('Enter'); await waitAtlas();
            }
            await waitAtlas();
            await page.waitForTimeout(1200);
            await page.evaluate(()=>{
                window.atlasPointerEvidence=[];
                for(const type of ['pointerdown','pointerup'])document.querySelector('canvas').addEventListener(type,event=>{
                    window.atlasPointerEvidence.push({type,at:event.timeStamp,observed_at:performance.now(),pointer:event.pointerId,button:event.button,trusted:event.isTrusted});
                },true);
            });
            await screenshot('01-inventory-1280');
            result.checks.push('Exact exported game starts directly in the authored Atlas independently of the editor');
            await identity();
            const first=page.locator('input[data-rvn-screen="atlas"][data-rvn-element="atlas_first_name"]');
            const last=page.locator('input[data-rvn-screen="atlas"][data-rvn-element="atlas_last_name"]');
            assert.equal(await first.inputValue(),'Alix');
            assert.equal(await last.inputValue(),'Vey');
            const bounds=await first.boundingBox(),canvas=await page.locator('canvas').boundingBox();
            assert.ok(Math.abs(bounds.x-(canvas.x+830*canvas.width/1920))<2);
            assert.ok(Math.abs(bounds.width-620*canvas.width/1920)<2);
            result.input_bounds={bounds,canvas};
            await first.click(); await first.fill('Éloïse Q'); await last.click(); await last.fill('Des Brumes');
            await first.click(); await first.press('q');
            assert.equal(await first.inputValue(),'Éloïse Qq','Q must edit the real text field, not navigate the Atlas');
            await first.fill('Éloïse Q');
            for(let attempt=0;attempt<2;attempt++){
                await first.focus();await first.press('F8',{delay:100});
                const closePreferences=page.getByRole('button',{name:/Fermer \(Échap \/ F8\)|Close \(Escape \/ F8\)/});
                await closePreferences.waitFor({state:'visible',timeout:10000});
                if(attempt===0)await screenshot('13-accessibility-from-native-input');
                await key('Escape');
                await closePreferences.waitFor({state:'hidden',timeout:10000});
                await first.waitFor({state:'visible',timeout:10000});
                assert.equal(await first.inputValue(),'Éloïse Q','F8 preferences may not lose the current native binding');
                assert.equal(await last.inputValue(),'Des Brumes');
            }
            result.checks.push('F8 from the actual native input opens real accessibility preferences twice, closes without story advance and preserves Unicode bindings');
            const origin=page.locator('select[aria-label="Origine du cartographe"]');
            await origin.selectOption('hautes_terres',{force:true});
            assert.equal(await origin.inputValue(),'hautes_terres');
            result.checks.push('Real Unicode native fields retain caret/editing, Q does not navigate, semantic origin selector writes its stable value');
            await mouse(945,681); // Actual on-canvas reduce-motion toggle.
            await mouse(1260,681); // Actual on-canvas high-contrast toggle.
            await mouse(1380,741); // Actual on-canvas slider.
            await screenshot('02-identity-1280');
            await page.setViewportSize({width:1920,height:1080}); await page.waitForTimeout(700);
            const bigBounds=await first.boundingBox();
            assert.ok(bigBounds.x>=0&&bigBounds.x+bigBounds.width<=1920);
            await screenshot('03-identity-1920');
            await page.setViewportSize({width:1280,height:720}); await page.waitForTimeout(700);
            await tab(1);
            await first.waitFor({state:'detached',timeout:15000});
            assert.equal(await first.count(),0,'Computed identity controls must disappear on inventory page');
            // Empty lantern then initial potion; hit geometry uses real authored page.
            await mouse(380+48+177+81,170+131+60); await mouse(1308,852);
            await mouse(380+48+81,170+131+132+60); await mouse(1308,852); await mouse(1308,852);
            await screenshot('04-inventory-used');
            await tab(2);
            const landmark=places.find(place=>place.id==='observatoire');
            const marker={x:380+58+landmark.x*1044/1020,y:170+124+landmark.y*522/530};
            await move(1740,940); const beforeHover=await screenshot('05-map-before-tooltip');
            await move(marker.x,marker.y); const hovered=await screenshot('06-map-tooltip');
            const left=380+Math.min(770,58+landmark.x*1044/1020-75),top=170+Math.max(131,124+landmark.y*522/530-93);
            const a=await point(left,top),b=await point(left+295,top+58);
            let changed=0;
            for(let y=Math.floor(a.y);y<Math.floor(b.y);y++)for(let x=Math.floor(a.x);x<Math.floor(b.x);x++){
                const p=(y*hovered.width+x)*4;
                if(Math.abs(hovered.data[p]-beforeHover.data[p])+Math.abs(hovered.data[p+1]-beforeHover.data[p+1])+Math.abs(hovered.data[p+2]-beforeHover.data[p+2])>20)changed++;
            }
            assert.ok(changed>400,'Real map hover must visibly draw its tooltip in the authored rectangle');
            result.tooltip_changed_pixels=changed;
            await mouse(marker.x,marker.y); await mouse(1308,852); await waitAtlas();
            result.checks.push('Actual Canvas map hit regions draw a hover tooltip and refuse a locked observatory trip');
            const forest=places.find(place=>place.id==='clairiere');
            await mouse(380+58+forest.x*1044/1020,170+124+forest.y*522/530); await mouse(1308,852);
            await waitNoAtlas();
            await page.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>node.textContent.includes('La forêt se referme')),undefined,{timeout:15000});
            await screenshot('07-immediate-travel');
            const saved=await quicksave(); result.saved=saved;
            assert.equal(variable(saved,'first_name'),'Éloïse Q');
            assert.equal(variable(saved,'last_name'),'Des Brumes');
            assert.equal(variable(saved,'origin'),'hautes_terres');
            assert.equal(variable(saved,'atlas_reduced_motion'),true);
            assert.equal(variable(saved,'atlas_high_contrast'),true);
            assert.ok(variable(saved,'atlas_text_scale')>1.0&&variable(saved,'atlas_text_scale')<=1.25);
            assert.equal(variable(saved,'atlas_health'),6);
            assert.equal(scalar(variable(saved,'atlas_quantities')[6]),2,'First potion consumes one dose, second at full health must not');
            assert.equal(variable(saved,'atlas_location'),'clairiere');
            assert.equal(variable(saved,'atlas_destination'),'clairiere');
            assert.ok(!saved.ui.screens.some(screen=>screen.name==='atlas'));
            result.checks.push('Travel reaches real narrative without an extra click; real F5 outside Atlas serializes identity, preferences, potion quantities and destination');
            await reopenAfterForest(); await identity();
            await first.fill('Changed after save');
            await key('Escape'); await waitNoAtlas();
            // Exercise the exact regression: the accessible narrative choice
            // owns DOM focus, but F6 still belongs to RVN, not the address bar.
            await page.getByRole('button',{name:/Rouvrir l’Atlas/}).focus();
            await page.waitForTimeout(600);
            result.f6_focus=await page.evaluate(()=>({active:document.activeElement?.getAttribute('aria-label'),tag:document.activeElement?.tagName,choices:Array.from(document.querySelectorAll('button[data-rvn-accessibility]')).filter(node=>!node.hidden).map(node=>({id:node.id,label:node.getAttribute('aria-label')}))}));
            assert.match(result.f6_focus.active??'',/Rouvrir l’Atlas/,'The real accessible choice must retain DOM focus before the shortcut');
            await page.keyboard.press('F6',{delay:100}); await page.waitForTimeout(350);
            await page.getByRole('button',{name:/Confirmer|Confirm/i}).waitFor({state:'visible',timeout:10000});
            await screenshot('08-load-confirmation');
            await key('ArrowLeft'); await key('Enter');
            await page.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>node.textContent.includes('La forêt se referme')),undefined,{timeout:15000});
            await reopenAfterForest(); await identity();
            assert.equal(await first.inputValue(),'Éloïse Q');
            assert.equal(await last.inputValue(),'Des Brumes');
            await screenshot('09-restored-identity');
            result.checks.push('Actual F6 confirmation reloads the browser save and restores native fields after later edits');
            // Turn motion back on through the actual control, then choose the
            // real introduction outside Atlas. This watches native browser
            // decoding; it never seeks, fabricates end feedback or calls RVN.
            await mouse(945,681);
            await key('Escape'); await waitNoAtlas();
            const introduction=page.getByRole('button',{name:/Voir l’ouverture animée/});
            await introduction.waitFor({state:'visible'});
            await page.evaluate(()=>{
                window.atlasVideoEvidence={frames:0,events:[],samples:[],sawEnd:false};
                const evidence=window.atlasVideoEvidence;
                const watch=video=>{
                    if(video.dataset.atlasQaObserved)return;
                    video.dataset.atlasQaObserved='true';
                    for(const type of ['loadedmetadata','loadeddata','playing','ended','error'])video.addEventListener(type,()=>{
                        evidence.events.push({type,time:video.currentTime,readyState:video.readyState});
                        if(type==='ended')evidence.sawEnd=true;
                    });
                    if(video.requestVideoFrameCallback){
                        const frame=(_now,metadata)=>{
                            evidence.frames++;
                            if(evidence.samples.length<64)evidence.samples.push({time:metadata.mediaTime,width:metadata.width,height:metadata.height,presentedFrames:metadata.presentedFrames});
                            if(video.isConnected&&!video.ended)video.requestVideoFrameCallback(frame);
                        };
                        video.requestVideoFrameCallback(frame);
                    }
                };
                window.atlasVideoObserver=new MutationObserver(()=>document.querySelectorAll('video').forEach(watch));
                window.atlasVideoObserver.observe(document.body,{childList:true,subtree:true});
                document.querySelectorAll('video').forEach(watch);
            });
            // The semantic button is intentionally clipped for screen readers.
            // Its DOM activation is the shipped accessible action path.
            await introduction.evaluate(node=>node.click());
            await page.waitForFunction(()=>document.querySelector('video'),undefined,{timeout:15000});
            const resume=page.getByRole('button',{name:/Cliquer pour lire la vidéo|Click to play video/});
            const playbackDeadline=Date.now()+20000;
            while(Date.now()<playbackDeadline){
                if(await resume.isVisible().catch(()=>false)){
                    result.video_user_resume=true;
                    await resume.click();
                }
                if(await page.evaluate(()=>Array.from(document.querySelectorAll('video')).some(video=>video.readyState>=2&&!video.paused&&video.currentTime>0.4)))break;
                await page.waitForTimeout(150);
            }
            await page.waitForFunction(()=>Array.from(document.querySelectorAll('video')).some(video=>video.readyState>=2&&!video.paused&&video.currentTime>=0.5),undefined,{timeout:10000});
            const firstCapture=await videoScreenshot('10-video-subtitle-one',0.5,1.9);
            result.video_first=firstCapture.observed;
            assert.equal(result.video_first.width,1280); assert.equal(result.video_first.height,720);
            assert.ok(result.video_first.duration>5.9&&result.video_first.duration<6.2);
            const subtitleOne=firstCapture.png;
            result.video_time_after_first_capture=await page.locator('video').first().evaluate(video=>video.currentTime).catch(()=>null);
            const secondCapture=await videoScreenshot('11-video-subtitle-two',2.2,3.9);
            result.video_second_time=secondCapture.observed.time;
            result.video_second=secondCapture.observed;
            const subtitleTwo=secondCapture.png;
            function captionPixels(png){
                let white=0;
                for(let y=650;y<705;y++)for(let x=120;x<1160;x++){
                    const i=(y*png.width+x)*4;
                    if(png.data[i]>215&&png.data[i+1]>215&&png.data[i+2]>215)white++;
                }
                return white;
            }
            result.video_caption_white_pixels=[captionPixels(subtitleOne),captionPixels(subtitleTwo)];
            assert.ok(result.video_caption_white_pixels.every(pixels=>pixels>120),'Both real rendered subtitle bands must contain visible lettering');
            await page.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>!node.hidden&&node.textContent.includes('La brume avale les routes, jamais les promesses')),undefined,{timeout:20000});
            await page.waitForFunction(()=>document.querySelectorAll('video').length===0,undefined,{timeout:5000});
            result.video_evidence=await page.evaluate(()=>{window.atlasVideoObserver.disconnect();return window.atlasVideoEvidence;});
            assert.ok(result.video_evidence.sawEnd,'The browser must really reach ended before RVN resumes');
            assert.ok(result.video_evidence.frames>=3,'Real decoded video frames must be presented');
            assert.ok(result.video_evidence.samples.some(frame=>frame.time>2),'Video time must progress across both subtitles');
            await screenshot('12-video-finished-dialogue');
            result.checks.push('Actual introduction choice decodes 1280×720 VP8/Vorbis in HTMLVideoElement, presents frames and two subtitles, reaches ended and returns to real dialogue without fabricated feedback');
            assert.deepEqual(result.errors,[],'No fatal runtime/browser error');
            // Bevy probes optional loader metadata; absence selects defaults.
            // menus.rvnui is also optional: this project uses programmable RVN.
            result.optional_not_found=[...new Set(result.not_found.filter(path=>path.endsWith('.meta')||path==='/menus.rvnui'))];
            const missingRequired=result.not_found.filter(path=>path!=='/favicon.ico'&&!path.endsWith('.meta')&&path!=='/menus.rvnui');
            result.required_not_found=[...new Set(missingRequired)];
            assert.deepEqual(missingRequired,[],'Export may not request absent resources');
            result.pointer_evidence=await page.evaluate(()=>window.atlasPointerEvidence);
            const down=new Map();result.pointer_observed_hold_ms=[];
            for(const event of result.pointer_evidence){
                if(event.type==='pointerdown')down.set(event.pointer,event);
                else if(down.has(event.pointer)){
                    result.pointer_observed_hold_ms.push(event.at-down.get(event.pointer).at);down.delete(event.pointer);
                }
            }
            assert.ok(result.pointer_evidence.length>0&&result.pointer_evidence.every(event=>event.trusted),'Canvas pointer evidence must come from actual trusted browser input');
            result.checks.push('All required authored resources load and no fatal browser diagnostic occurs; optional metadata/menu probes are listed separately');
            result.status='passed'; console.log(`${name} ${result.version}: PASS (${result.checks.length} checks)`);
        } catch(error) {
            result.status='failed'; result.failure=String(error); console.error(`${name}: ${error}`);
            result.semantic=await page.locator('[data-rvn-accessibility]').allTextContents().catch(()=>[]);
            result.active_element=await page.evaluate(()=>({tag:document.activeElement?.tagName,label:document.activeElement?.getAttribute('aria-label'),id:document.activeElement?.id})).catch(()=>null);
            result.pointer_evidence=await page.evaluate(()=>window.atlasPointerEvidence??[]).catch(()=>[]);
            result.video_failure_evidence=await page.evaluate(()=>({decode:window.atlasVideoEvidence??null,players:Array.from(document.querySelectorAll('video')).map(video=>({time:video.currentTime,duration:video.duration,paused:video.paused,readyState:video.readyState,width:video.videoWidth,height:video.videoHeight}))})).catch(()=>null);
            await page.screenshot({path:join(evidence,`${name}-failure.png`)}).catch(()=>{});
        } finally { await context.close(); await browser.close(); }
    }
} finally {
    server.close(); results.finished_at=new Date().toISOString();
    writeFileSync(join(evidence,'web-evidence.json'),JSON.stringify(results,null,2)+'\n');
}
if(results.browsers.some(result=>result.status!=='passed'))process.exitCode=1;
