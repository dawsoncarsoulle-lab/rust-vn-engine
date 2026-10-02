// Isolated browser-adapter tests, not a substitute for real browser playback.
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const rust=readFileSync(new URL('../rvn_bevy/src/web_video.rs',import.meta.url),'utf8');
const source=rust.split('inline_js=r#"')[1].split('"#)]')[0].replaceAll('export function','function');
let refusal=false,range=true,oversized=false;
const revoked=[];let blobs=0;
class MediaURL extends URL{static createObjectURL(){return `blob:fixture-${++blobs}`;}static revokeObjectURL(url){revoked.push(url);}}
const elements=[];
function element(kind){
    const result={kind,style:{},listeners:{},paused:true,currentTime:0,readyState:2,videoWidth:320,videoHeight:180,duration:2,loop:false,seeking:false,
        setAttribute(){},removeAttribute(){},load(){},remove(){this.removed=true;},addEventListener(name,callback){this.listeners[name]=callback;},pause(){this.paused=true;},
        play(){if(refusal)return Promise.reject(Object.assign(new Error('Click required'),{name:'NotAllowedError'}));this.paused=false;return Promise.resolve();},
        getContext(){return{drawImage(){},getImageData(){return{data:new Uint8ClampedArray(320*180*4)}}};}};
    elements.push(result);return result;
}
const context=vm.createContext({URL:MediaURL,Blob,Uint8Array,Uint8ClampedArray,AbortController,Promise,console,
    document:{hidden:false,baseURI:'https://example.test/',createElement:element,body:{appendChild(){}},addEventListener(){}},
    fetch:async()=>({ok:true,status:range?206:200,headers:{get(name){return name==='Content-Length'&&oversized?'100000000':null;}},body:{getReader(){return{async read(){return{done:true}},async cancel(){}}}}})});
vm.runInContext(source,context);
const view=(playback,epoch=1)=>[{id:'intro',source:'clip.webm',mask:null,position:0,playback,epoch,looping:false,volume:1}];
const sync=(state,epoch=1)=>context.rvn_video_sync(JSON.stringify(view(state,epoch)),true,1,'Click to play video',()=>null);
const settle=async()=>{for(let i=0;i<40;i++)await Promise.resolve();};
sync('blocked');await settle();
let button=elements.find(element=>element.kind==='button'&&!element.removed);
assert.ok(button&&!button.hidden,'A restored blocked clip needs a visible resume control');
assert.ok(elements.find(element=>element.kind==='video').paused);
button.listeners.click({stopPropagation(){}});await settle();
assert.equal(JSON.parse(context.rvn_video_reports()).find(report=>report.kind==='resume').epoch,1);
sync('playing');await settle();assert.ok(!elements.find(element=>element.kind==='video').paused);
context.rvn_video_sync('[]',true,1,'',()=>null);
refusal=true;sync('playing');await settle();sync('playing');await settle();
// A queued rejection reports the current epoch, not an obsolete closure epoch.
sync('playing',2);await settle();
const blocked=JSON.parse(context.rvn_video_reports()).find(report=>report.kind==='blocked');
assert.ok(blocked,'An actual autoplay refusal must be reported');
sync('blocked',2);button=elements.find(element=>element.kind==='button'&&!element.removed);assert.ok(button&&!button.hidden);
refusal=false;button.listeners.click({stopPropagation(){}});await settle();
assert.equal(JSON.parse(context.rvn_video_reports()).find(report=>report.kind==='resume').epoch,2);
sync('playing',3);await settle();assert.ok(!elements.filter(element=>element.kind==='video'&&!element.removed)[0].paused);
context.rvn_video_sync('[]',false,1,'',()=>null);
assert.ok(elements.filter(element=>element.kind==='video'||element.kind==='button').every(element=>element.removed));
range=false;sync('paused');await settle();sync('paused');await settle();
assert.ok(elements.filter(element=>element.kind==='video'&&!element.removed)[0].src.startsWith('blob:'),'A server without ranges needs a seekable local resource');
const seek=view('paused',4);seek[0].position=0.8;context.rvn_video_sync(JSON.stringify(seek),true,1,'',()=>null);
assert.equal(elements.filter(element=>element.kind==='video'&&!element.removed)[0].currentTime,0.8);
context.rvn_video_sync('[]',false,1,'',()=>null);assert.equal(revoked.length,1);
oversized=true;sync('paused');await settle();await settle();
assert.ok(JSON.parse(context.rvn_video_reports()).some(report=>report.kind==='error'&&report.message.includes('byte-range')));
context.rvn_video_sync('[]',false,1,'',()=>null);
console.log('PASS: restored autoplay refusal, epoch-safe events, bounded HTTP seek fallback and disposal');
