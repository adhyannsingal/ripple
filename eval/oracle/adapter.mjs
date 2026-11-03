// Evaluation only: no project code/config plugins are executed.
import ts from 'typescript';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
export const POLICY = 'ripple-oracle-v1';
export function byteOffset(text, offset) {
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length) throw Error('invalid UTF-16 offset');
  if (offset && offset < text.length && /[\uD800-\uDBFF]/u.test(text[offset-1]) && /[\uDC00-\uDFFF]/u.test(text[offset])) throw Error('offset bisects surrogate pair');
  return Buffer.byteLength(text.slice(0, offset), 'utf8');
}
export function byteSpan(text, start, end) {
  if (end < start) throw Error('reversed span');
  return {start: byteOffset(text,start), end: byteOffset(text,end)};
}
export function mapSpan(symbols, file, span, exact=false) {
  const matches = symbols.filter(s => s.file===file && (exact ? s.span.start===span.start && s.span.end===span.end : s.span.start<=span.start && s.span.end>=span.end));
  if (!matches.length) return {reason:'unmatched_span'};
  const shortest = Math.min(...matches.map(s=>s.span.end-s.span.start));
  const best=matches.filter(s=>s.span.end-s.span.start===shortest);
  return best.length===1 ? {symbol:best[0]} : {reason:'ambiguous_span'};
}
export function uniqueEdges(edges) {
  const map=new Map();
  for (const e of edges) {
    const key=JSON.stringify([e.source,e.target,e.kind]);
    if (!map.has(key)) map.set(key,{...e,occurrences:[]});
    const item=map.get(key);
    for(const loc of e.occurrences ?? [e.location]) if (loc && !item.occurrences.some(x=>JSON.stringify(x)===JSON.stringify(loc))) item.occurrences.push(loc);
  }
  return [...map.entries()].sort(([a],[b])=>a.localeCompare(b,'en')).map(([,e])=>({...e,occurrences:e.occurrences.sort((a,b)=>a.file<b.file?-1:a.file>b.file?1:a.span.start-b.span.start)}));
}
function topFunction(n) { return ts.isFunctionDeclaration(n) && ts.isSourceFile(n.parent) && n.name && n.body; }
function supportedClass(n) { return ts.isClassDeclaration(n) && ts.isSourceFile(n.parent) && n.name; }
function method(n) { return ts.isMethodDeclaration(n) && supportedClass(n.parent) && ts.isIdentifier(n.name) && n.body; }
function definition(n) { return topFunction(n) || supportedClass(n) || method(n); }
function boundaryOwner(n) {
  for(let p=n.parent;p&&!ts.isSourceFile(p);p=p.parent) {
    // Arrows/callbacks and unsupported containers do not gain invented owners.
    if (ts.isFunctionLike(p) || ts.isClassLike(p) || ts.isModuleDeclaration(p)) return definition(p) ? p : null;
  }
  return undefined;
}
function inType(n) { for(let p=n.parent;p&&!ts.isStatement(p);p=p.parent) if(ts.isTypeNode(p)) return true; return false; }
function runtimeIdentifier(n) {
  if(!ts.isIdentifier(n)||inType(n)) return false;
  const p=n.parent;
  if(ts.isImportSpecifier(p)||ts.isImportClause(p)||ts.isNamespaceImport(p)||ts.isExportSpecifier(p)||ts.isImportEqualsDeclaration(p)) return false;
  if ((p.name===n && !ts.isShorthandPropertyAssignment(p)) || (ts.isPropertyAccessExpression(p)&&p.name===n) || ts.isQualifiedName(p)) return false;
  if(ts.isLabeledStatement(p)||ts.isBreakOrContinueStatement(p)) return false;
  return true;
}
export function analyze(root, files) {
  root=fs.realpathSync(root); files=[...files].sort();
  if(new Set(files).size!==files.length || files.length>1000) throw Error('duplicate/too many source files');
  const inputHashes={};
  for(const f of files) {
    if(!/\.(ts|tsx)$/.test(f)||path.isAbsolute(f)||f.split(/[\\/]/).some(p=>p==='..'||p===''||p==='.')) throw Error('invalid source path');
    const absolute=path.join(root,f); if(fs.realpathSync(absolute)!==absolute) throw Error('symlink source rejected');
    const bytes=fs.readFileSync(absolute);if(bytes.length>4*1024*1024) throw Error('source too large');
    if(!Buffer.from(bytes.toString('utf8')).equals(bytes)) throw Error('non-UTF8 source');
    inputHashes[f]=crypto.createHash('sha256').update(bytes).digest('hex');
  }
  const allowed=new Set(files.map(f=>path.join(root,f)));
  const options={target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,moduleResolution:ts.ModuleResolutionKind.Node10,jsx:ts.JsxEmit.Preserve,noEmit:true,noLib:true,types:[],skipLibCheck:true};
  const host=ts.createCompilerHost(options);host.readFile=f=>allowed.has(path.resolve(f))?fs.readFileSync(f,'utf8'):undefined;host.fileExists=f=>allowed.has(path.resolve(f));
  const program=ts.createProgram([...allowed],options,host), checker=program.getTypeChecker();
  const sources=program.getSourceFiles().filter(s=>allowed.has(path.resolve(s.fileName)));
  const relative=s=>path.relative(root,s.fileName).split(path.sep).join('/');
  const symbols=[], defs=new Map();
  const span=n=>byteSpan(n.getSourceFile().text,n.getStart(),n.end);
  for(const sf of sources) {
    const visit=n=>{
      if(definition(n)) {
        const name=method(n)?`${n.parent.name.text}.${n.name.text}`:n.name.text;
        const implementationSpan=span(n);
        const siblings=topFunction(n)?(checker.getSymbolAtLocation(n.name)?.getDeclarations()??[]).filter(d=>ts.isFunctionDeclaration(d)&&d.getSourceFile()===sf):[n];
        const groupedSpan={start:Math.min(implementationSpan.start,...siblings.map(d=>span(d).start)),end:Math.max(implementationSpan.end,...siblings.map(d=>span(d).end))};
        const entry={file:relative(sf),name,kind:supportedClass(n)?'CLASS':method(n)?'METHOD':'FUNCTION',span:groupedSpan,implementation_span:implementationSpan,name_span:span(n.name)};
        // Use path + declaration byte span, independent of Ripple extraction success/identity.
        entry.key=JSON.stringify([entry.file,entry.kind,entry.span.start,entry.span.end]);symbols.push(entry);defs.set(n,entry);
      }
      ts.forEachChild(n,visit);
    }; visit(sf);
  }
  const edges=[], exclusions=[], occurrences=[];
  for(const sf of sources) {
    const visit=n=>{
      if(runtimeIdentifier(n)) {
        const location={file:relative(sf),span:span(n)}, owner=boundaryOwner(n);
        const record={location,text:n.text};
        let symbol=checker.getSymbolAtLocation(n);
        if(symbol?.flags & ts.SymbolFlags.Alias) symbol=checker.getAliasedSymbol(symbol);
        const declarations=symbol?.getDeclarations()??[];
        const targets=[...new Set(declarations.filter(d=>definition(d)).map(d=>defs.get(d)).filter(Boolean))];
        let reason;
        const call=ts.isCallExpression(n.parent)&&n.parent.expression===n;
        if(owner===null) reason='unsupported_owner';
        else if(ts.isCallExpression(n.parent)&&n.parent.questionDotToken) reason='optional_call';
        else if(ts.isNewExpression(n.parent)) reason='constructor';
        else if(targets.length!==1) reason=targets.length>1?'ambiguous_definition':'outside_supported_definition';
        else if(call&&targets[0].kind!=='FUNCTION') reason='non_function_call';
        if(reason) exclusions.push({...record,reason});
        else {
          const source=owner?defs.get(owner).key:JSON.stringify([relative(sf),'FILE']);
          const edge={source,target:targets[0].key,kind:call?'CALLS':'REFERENCES',location};edges.push(edge);occurrences.push({...record,source,target:targets[0].key,kind:edge.kind});
        }
      }
      ts.forEachChild(n,visit);
    };visit(sf);
  }
  const diagnostics=[...program.getSyntacticDiagnostics(),...program.getSemanticDiagnostics()].map(d=>({file:d.file?relative(d.file):null,code:d.code,start:d.file&&d.start!=null?byteOffset(d.file.text,d.start):null,message:ts.flattenDiagnosticMessageText(d.messageText,' ')}));
  // A syntax error invalidates this run rather than silently lowering the denominator.
  if(program.getSyntacticDiagnostics().length) throw Error('corpus has syntax errors');
  return {format_version:1,policy:POLICY,typescript:ts.version,node:process.version,options,input_hashes:inputHashes,symbols:symbols.sort((a,b)=>a.key<b.key?-1:a.key>b.key?1:0),edges:uniqueEdges(edges),occurrences,exclusions,diagnostics,coverage:{runtime_identifiers:occurrences.length+exclusions.length,eligible_occurrences:occurrences.length,eligible_edges:uniqueEdges(edges).length,excluded_occurrences:exclusions.length}};
}
if(process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  try {
    const [root,list]=process.argv.slice(2);if(!root||!list)throw Error('usage: node adapter.mjs ROOT FILE-LIST.json');
    console.log(JSON.stringify(analyze(root,JSON.parse(fs.readFileSync(list,'utf8'))),null,2));
  } catch(e) {console.error(e.message);process.exitCode=1;}
}
