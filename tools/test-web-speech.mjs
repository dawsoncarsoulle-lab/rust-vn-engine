// Exercises the actual browser adapter without making a machine speak.
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const source=readFileSync(new URL('../rvn_bevy/src/speech_web.rs',import.meta.url),'utf8').match(/inline_js=r#"([\s\S]*?)"#/)[1].replaceAll('export ','');
let voices=[{lang:'fr-FR',default:true},{lang:'en-US',default:false}], utterances=[],cancelled=0,listener=null,timer=null;
const context=vm.createContext({window:{SpeechSynthesisUtterance:function(text){this.text=text;},speechSynthesis:{cancel(){cancelled++;},getVoices(){return voices;},speak(utterance){utterances.push(utterance);utterance.onstart?.();},addEventListener(_,fn){listener=fn;},removeEventListener(_,fn){if(listener===fn)listener=null;}}},SpeechSynthesisUtterance:function(text){this.text=text;},document:{documentElement:{lang:'en'}},navigator:{language:'en-US'},setTimeout(fn){timer=fn;return 1;},clearTimeout(){timer=null;}});
vm.runInContext(source,context);
const reports=[];const report=message=>reports.push(message);
await context.rvnSpeechSpeak('<literal> Éléonore',1.2,0.3,'fr-CA',report);
assert.equal(utterances.length,1);assert.equal(utterances[0].text,'<literal> Éléonore');assert.equal(utterances[0].voice.lang,'fr-FR');assert.equal(utterances[0].rate,1.2);assert.equal(utterances[0].volume,0.3);assert.deepEqual(reports,['']);
await context.rvnSpeechSpeak('Missing voice',1,1,'zz-QQ',report);assert.match(reports.at(-1),/No browser speech voice/);assert.equal(utterances.length,1);
const old=utterances[0];context.rvnSpeechStop();old.onerror({error:'audio-busy'});assert.equal(reports.length,2);
voices=[];const waiting=context.rvnSpeechSpeak('Delayed voice',1,1,'en',report);assert.ok(listener);voices=[{lang:'en-US',default:true}];listener();await waiting;assert.equal(utterances.length,2);
voices=[];const cancelledWait=context.rvnSpeechSpeak('Stale',1,1,'en',report);context.rvnSpeechStop();voices=[{lang:'en-US'}];listener();await cancelledWait;assert.equal(utterances.length,2);
voices=[];const unavailable=context.rvnSpeechSpeak('No voice',1,1,'en',report);timer();await unavailable;assert.match(reports.at(-1),/No browser speech voice/);
context.window.speechSynthesis.getVoices=()=>{throw new Error('Service unavailable');};await context.rvnSpeechSpeak('Failure',1,1,'en',report);assert.match(reports.at(-1),/Service unavailable/);
assert.ok(cancelled>=6);console.log('PASS: language selection, literal Unicode, rate/volume, delayed/no voices, cancellation epochs and service exceptions');
