// Read-only browser diagnosis of semantic choice focus against a real export.
// No calls into RVN, no synthetic scene state, no mutation of shipped controls.
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve,join,extname} from 'node:path';
import {createRequire} from 'node:module';
import {homedir} from 'node:os';
const [directory,output]=process.argv.slice(2),root=resolve(directory),evidence=resolve(output);
mkdirSync(evidence,{recursive:true});
const require=createRequire(import.meta.url);
const playwright=require(join(homedir(),'.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright'));
const mime={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.rvn':'text/plain','.ttf':'font/ttf','.webm':'video/webm'};
const server=createServer((request,response)=>{
    try{
        const path=decodeURIComponent(new URL(request.url,'http://localhost').pathname),file=resolve(root,'.'+(path==='/'?'/index.html':path));
        if(!file.startsWith(root+'/'))throw new Error('outside export');
        const bytes=readFileSync(file);response.writeHead(200,{'Content-Type':mime[extname(file)]||'application/octet-stream'});response.end(bytes);
    }catch{response.writeHead(404);response.end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const records=[];
try{
    for(const name of ['firefox','chrome']){
        const browser=name==='chrome'?await playwright.chromium.launch({executablePath:'/usr/bin/google-chrome',headless:true,args:['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader']}):await playwright.firefox.launch({headless:true,firefoxUserPrefs:{'webgl.force-enabled':true,'webgl.disabled':false,'gfx.webrender.all':true,'gfx.webrender.software':true}});
        const page=await browser.newPage({viewport:{width:1280,height:720}}),record={name,version:browser.version()};records.push(record);
        try{
            await page.goto(`http://127.0.0.1:${server.address().port}/`,{waitUntil:'domcontentloaded'});
            await page.waitForFunction(()=>Array.from(document.querySelectorAll('[data-rvn-accessibility]')).some(node=>!node.hidden&&node.textContent.includes('Échap · Retour')),undefined,{timeout:60000});
            await page.locator('canvas').focus();await page.keyboard.press('Escape',{delay:100});
            const choice=page.getByRole('button',{name:/Rouvrir l’Atlas/});await choice.waitFor({state:'visible',timeout:15000});
            await page.evaluate(()=>{
                const node=Array.from(document.querySelectorAll('button[data-rvn-accessibility]')).find(node=>!node.hidden&&node.getAttribute('aria-label')?.includes('Rouvrir l’Atlas'));
                window.focusDiagnostic={node,events:[],snapshots:[]};
                const d=window.focusDiagnostic;
                const describe=target=>({tag:target?.tagName,id:target?.id,label:target?.getAttribute?.('aria-label'),hidden:target?.hidden,connected:target?.isConnected,same:target===node});
                for(const type of ['focusin','focusout','blur'])document.addEventListener(type,event=>d.events.push({kind:type,time:performance.now(),target:describe(event.target),related:describe(event.relatedTarget)}),true);
                const focus=HTMLElement.prototype.focus;
                HTMLElement.prototype.focus=function(...args){d.events.push({kind:'focus-call',time:performance.now(),target:describe(this),stack:new Error().stack});return focus.apply(this,args);};
                d.observer=new MutationObserver(mutations=>{
                    for(const mutation of mutations){
                        if(mutation.type==='attributes'&&mutation.target===node&&['hidden','disabled','id'].includes(mutation.attributeName))d.events.push({kind:'attribute',name:mutation.attributeName,time:performance.now(),target:describe(node)});
                        for(const removed of mutation.removedNodes)if(removed===node||removed.contains?.(node))d.events.push({kind:'removed',time:performance.now(),target:describe(node)});
                        for(const added of mutation.addedNodes)if(added===node)d.events.push({kind:'reinserted',time:performance.now(),target:describe(node)});
                    }
                });
                d.observer.observe(document.body,{childList:true,subtree:true,attributes:true,attributeFilter:['hidden','disabled','id']});
            });
            await choice.focus();
            for(const delay of [0,25,125,450,1000]){
                if(delay)await page.waitForTimeout(delay);
                await page.evaluate(()=>{
                    const d=window.focusDiagnostic,current=Array.from(document.querySelectorAll('button[data-rvn-accessibility]')).find(node=>!node.hidden&&node.getAttribute('aria-label')?.includes('Rouvrir l’Atlas'));
                    d.snapshots.push({time:performance.now(),original:{id:d.node.id,hidden:d.node.hidden,disabled:d.node.disabled,connected:d.node.isConnected},current:{id:current?.id,same:current===d.node},active:{tag:document.activeElement.tagName,id:document.activeElement.id,label:document.activeElement.getAttribute('aria-label')}});
                });
            }
            record.trace=await page.evaluate(()=>{const d=window.focusDiagnostic;d.observer.disconnect();return {events:d.events,snapshots:d.snapshots};});
            await page.screenshot({path:join(evidence,`${name}.png`)});
            console.log(JSON.stringify({name,snapshots:record.trace.snapshots}));
        }catch(error){record.error=String(error);console.error(record.error);}
        finally{await browser.close();}
    }
}finally{server.close();writeFileSync(join(evidence,'focus.json'),JSON.stringify(records,null,2)+'\n');}
