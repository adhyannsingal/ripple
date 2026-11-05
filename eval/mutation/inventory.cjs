// Independent compiler-AST static import baseline for the owned fixture only.
const fs = require('node:fs');
const path = require('node:path');
const ts = require(path.resolve(process.argv[3]));
if (ts.version !== '6.0.3') throw new Error('expected pinned TypeScript 6.0.3');
const root = process.argv[2];
const files = fs.readdirSync(root).filter(p => p.endsWith('.ts')).sort();
const imports = {};
for (const file of files) {
  const source = ts.createSourceFile(file, fs.readFileSync(path.join(root,file),'utf8'),ts.ScriptTarget.Latest,true);
  if (source.parseDiagnostics.length) throw new Error('invalid baseline syntax');
  imports[file] = [];
  for (const node of source.statements) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
      const text = node.moduleSpecifier.text;
      if (!text.startsWith('./')) throw new Error('fixture must use local imports');
      const target = text.slice(2) + '.ts';
      if (!files.includes(target)) throw new Error('missing static fixture target');
      imports[file].push(target);
    }
  }
}
console.log(JSON.stringify({typescript:ts.version, imports, tests:files.filter(p=>p.endsWith('.test.ts'))}));
