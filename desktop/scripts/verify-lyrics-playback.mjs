// Manual regression probe: launches the real Windows player with an isolated profile,
// plays a local silent WAV through rodio, and observes natural transitions using live Kuwo lyrics.
// Requires the release build, WebView2 remote debugging on port 9222, and network access.
// Run from repo root: node desktop/scripts/verify-lyrics-playback.mjs <exe> --reduce-motion
// Add --animation-off to check that the explicit off setting still disables easing.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';import {createServer} from 'node:http';import fs from 'node:fs';import os from 'node:os';import path from 'node:path';import {chromium} from 'playwright';
const profile=fs.mkdtempSync(path.join(os.tmpdir(),'lyrics-playback-'));fs.mkdirSync(path.join(profile,'wcmusic'),{recursive:true});
fs.writeFileSync(path.join(profile,'wcmusic/settings.json'),JSON.stringify({dark_theme:true,lyrics_enabled:false,lyrics:{font_family:'MiSans',font_size:28,font_weight:700,animation:process.argv.includes('--animation-off')?'off':'slide',locked:true,window_height:420}}));
const rate=48000,duration=90,data=Buffer.alloc(rate*duration*4),wav=Buffer.alloc(44);wav.write('RIFF');wav.writeUInt32LE(36+data.length,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(2,22);wav.writeUInt32LE(rate,24);wav.writeUInt32LE(rate*4,28);wav.writeUInt16LE(4,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(data.length,40);const audio=Buffer.concat([wav,data]);
const server=createServer((req,res)=>{res.setHeader('Content-Type','audio/wav');res.setHeader('Content-Length',audio.length);res.end(audio)});await new Promise(r=>server.listen(1432,'127.0.0.1',r));
const log=fs.openSync('desktop/shots/playback-motion-runtime.log','a');const exe=process.argv[2]??'desktop/src-tauri/target/release/wcmusic-desktop.exe';
const child=spawn(path.resolve(exe),[],{env:{...process.env,APPDATA:profile,WEBVIEW2_USER_DATA_FOLDER:path.join(profile,'webview')},stdio:['ignore',log,log]});
const wait=ms=>new Promise(r=>setTimeout(r,ms));let browser;
try{
 for(let i=0;i<60;i++){try{await fetch('http://127.0.0.1:9222/json/list');break}catch{await wait(250)}}await wait(1500);
 browser=await chromium.connectOverCDP('http://127.0.0.1:9222');const ctx=browser.contexts()[0];let main;
 for(const p of ctx.pages())if(await p.evaluate(()=>window.__TAURI_INTERNALS__.metadata.currentWindow.label).catch(()=>null)==='main')main=p;
 if(process.argv.includes('--reduce-motion')) { await main.emulateMedia({reducedMotion:'reduce'}); await main.reload(); await wait(800); }
 const invoke=(name,args)=>main.evaluate(({name,args})=>window.__TAURI_INTERNALS__.invoke(name,args),{name,args});
 console.log('settings',await invoke('get_settings').then(s=>({animation:s.lyrics.animation,profile:profile})));
 await invoke('set_playback_volume',{volume:0});
 const script=`/**\n * @name Motion probe\n */\nconst {EVENT_NAMES,on,send}=lx;on(EVENT_NAMES.request,async()=> 'http://127.0.0.1:1432/silent.wav');send(EVENT_NAMES.inited,{sources:{kw:{name:'test',type:'music',actions:['musicUrl'],qualitys:['128k','320k','flac']}}});`;
 await invoke('import_source_script',{name:'Motion probe',script});
 const track={id:'kw-228908',sourceId:'228908',source:'kw',title:'晴天',artist:'周杰伦',album:'测试',durationMs:90000,uri:'',artworkUri:null,quality:null};
 const lyrics=await invoke('load_lyrics',{track});console.log('first lyrics',lyrics.slice(0,10));
 await invoke('play_track',{track});await invoke('toggle_lyrics_window');await wait(1800);
 await main.locator('footer button').first().click();await wait(1800);
 let lyric;for(const p of ctx.pages())if(await p.evaluate(()=>window.__TAURI_INTERNALS__.metadata.currentWindow.label).catch(()=>null)==='lyrics')lyric=p;
 if(process.argv.includes('--reduce-motion')) {
   await main.emulateMedia({reducedMotion:'reduce'});
   await lyric.emulateMedia({reducedMotion:'reduce'});
   await lyric.reload(); await wait(1000);
 }
 console.log('snapshot',await invoke('playback_snapshot'));
 const sample=(page,desktop)=>page.evaluate(desktop=>new Promise(resolve=>{
 const points=[],start=performance.now();let prev='';
 function tick(){const list=desktop?document.querySelector('[data-lyrics-list]'):document.querySelector('.lyric-line-current')?.parentElement;
 const value=list?(desktop?new DOMMatrixReadOnly(getComputedStyle(list).transform).m42:list.scrollTop):null;
 const index=desktop?document.querySelector('[aria-current=true]')?.dataset.lyricIndex:document.querySelector('.lyric-line-current')?.textContent;
 if(String(value)+index!==prev){points.push({t:Math.round(performance.now()-start),value,index});prev=String(value)+index;}
 if(performance.now()-start<14000)requestAnimationFrame(tick);else resolve({reduced:matchMedia('(prefers-reduced-motion: reduce)').matches,points});}tick();
 }),desktop);
 const result=await Promise.all([sample(main,false),sample(lyric,true)]);fs.writeFileSync('desktop/shots/playing-motion-samples.json',JSON.stringify(result,null,2));
 for(let i=0;i<2;i++) {
   const samples=result[i].points;
   const positions=new Set(samples.filter(p=>p.value!==null).map(p=>p.value));
   if(process.argv.includes('--animation-off')) assert.ok(positions.size<25,'off must switch immediately');
   else assert.ok(positions.size>60,'playing lyrics must contain continuous intermediate positions');
   console.log(i?'desktop':'detail','samples',samples.length,'unique positions',positions.size,'reduced',result[i].reduced);
 }
 console.log('PASS: natural playback transitions verified',process.argv.includes('--animation-off')?'without animation':'with easing');
}finally{await browser?.close();child.kill();server.close()}
