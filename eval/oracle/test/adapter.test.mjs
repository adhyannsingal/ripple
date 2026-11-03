import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {analyze,byteOffset,byteSpan,mapSpan,uniqueEdges} from '../adapter.mjs';
const root=fileURLToPath(new URL('../../corpora/mini/',import.meta.url));
const files=JSON.parse(fs.readFileSync(path.join(root,'files.json'),'utf8'));
test('UTF-16 offsets become exact UTF-8 byte spans, including astral/CRLF',()=>{
 const text='é😀\r\ncall'; assert.equal(byteOffset(text,3),6);assert.deepEqual(byteSpan(text,5,9),{start:8,end:12});assert.throws(()=>byteOffset(text,2),/surrogate/);assert.throws(()=>byteOffset(text,-1));assert.throws(()=>byteSpan(text,3,0));
});
test('mapping explicitly reports missing/ambiguous and picks nearest owner',()=>{
 const outer={file:'f.ts',span:{start:0,end:20}},inner={file:'f.ts',span:{start:4,end:10}};
 assert.equal(mapSpan([outer,inner],'f.ts',{start:5,end:6}).symbol,inner);
 assert.equal(mapSpan([inner,inner],'f.ts',{start:5,end:6}).reason,'ambiguous_span');
 assert.equal(mapSpan([outer],'x.ts',{start:5,end:6}).reason,'unmatched_span');
 assert.equal(mapSpan([outer],'f.ts',{start:5,end:6},true).reason,'unmatched_span');
});
test('mini corpus manually expected alias/barrel/overload/TSX edges and duplicate occurrences',()=>{
 const a=analyze(root,files); const names=new Map(a.symbols.map(s=>[s.key,`${s.file}::${s.name}`]));
 assert.deepEqual(a.edges.map(e=>[names.get(e.source),names.get(e.target),e.kind]).sort(),[
 ['main.ts::caller','lib.ts::café','CALLS'],['main.ts::reference','lib.ts::café','REFERENCES'],['view.tsx::View','main.ts::caller','CALLS']].sort());
 assert.deepEqual(a.symbols.find(s=>s.name==='café').span,{start:0,end:88});assert.equal(a.coverage.eligible_occurrences,4);assert.equal(a.coverage.eligible_edges,3);assert.equal(a.edges.find(e=>e.kind==='CALLS'&&names.get(e.source)==='main.ts::caller').occurrences.length,2);
 const text=fs.readFileSync(path.join(root,'main.ts'));for(const e of a.edges)for(const loc of e.occurrences)if(loc.file==='main.ts')assert.equal(text.subarray(loc.span.start,loc.span.end).toString(),'use');
 assert.ok(!a.edges.some(e=>names.get(e.source)==='main.ts::shadow'));assert.deepEqual(analyze(root,[...files].reverse()),a);
});
test('duplicate edges keep occurrence provenance',()=>{
 const e={source:'a',target:'b',kind:'CALLS',location:{file:'f',span:{start:1,end:2}}};assert.equal(uniqueEdges([e,e]).length,1);assert.equal(uniqueEdges([e,e])[0].occurrences.length,1);
});
test('unsupported callback and dynamic/method calls do not enter comparable scope',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'ripple-oracle-'));
 try {fs.writeFileSync(path.join(dir,'a.ts'),'function f() {}\nfunction g() { f?.(); const x=()=>f(); const obj={ f }; obj.f(); }'); const a=analyze(dir,['a.ts']);assert.ok(a.exclusions.some(e=>e.reason==='unsupported_owner'));assert.ok(a.exclusions.some(e=>e.reason==='optional_call'));assert.ok(!a.edges.some(e=>e.kind==='CALLS'));} finally{fs.rmSync(dir,{recursive:true,force:true});}
});
test('syntax errors/duplicate inventory fail instead of reducing denominator',()=>{
 assert.throws(()=>analyze(root,['lib.ts','lib.ts']));const dir=fs.mkdtempSync(path.join(os.tmpdir(),'ripple-oracle-'));try{fs.writeFileSync(path.join(dir,'a.ts'),'function {');assert.throws(()=>analyze(dir,['a.ts']),/syntax/);}finally{fs.rmSync(dir,{recursive:true,force:true});}
});
