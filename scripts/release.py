#!/usr/bin/env python3
"""Prepare and verify reviewable binary candidates; no public publication."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import tarfile
import shutil
import stat
import subprocess
import tempfile
import zipfile

ROOT=Path(__file__).resolve().parents[1]
def digest(raw):return hashlib.sha256(raw).hexdigest()
def command(argv):
    return subprocess.run([str(a)for a in argv],cwd=ROOT,capture_output=True,text=True,timeout=180,check=True).stdout
def safe(name):
    path=PurePosixPath(name)
    if not name or any(ord(c)<32 or ord(c)==127 for c in name) or path.is_absolute() or str(path)!=name or '\\' in name or ':' in name or any(p in ('','.','..') for p in name.split('/')):
        raise ValueError('unsafe archive path')
    return path

def prepare(binary,output,target):
    binary=binary.resolve(strict=True)
    if output.exists():raise ValueError('output must be new')
    metadata=json.loads(command(['cargo','metadata','--locked','--format-version','1']))
    version=next(p['version'] for p in metadata['packages'] if p['name']=='ripple-cli')
    actual=command([binary,'--version']).strip()
    if actual!='ripple '+version:raise ValueError('binary/source version mismatch')
    host=next(line.split(': ',1)[1] for line in command(['rustc','-vV']).splitlines() if line.startswith('host: '))
    if target!=host:raise ValueError('host-built artifact label mismatch')
    sqlite=next(p['id']for p in metadata['packages']if p['name']=='libsqlite3-sys')
    features=next(n['features']for n in metadata['resolve']['nodes']if n['id']==sqlite)
    if 'bundled' not in features:raise ValueError('bundled SQLite feature absent')
    source_commit=command(['git','rev-parse','HEAD']).strip()
    payload={binary.name:binary.read_bytes()}
    for name in ['LICENSE','CHANGELOG.md']:payload[name]=(ROOT/name).read_bytes()
    payload['SOURCE-README.md']=(ROOT/'README.md').read_bytes()
    docs='https://github.com/adhyannsingal/ripple/blob/'+source_commit+'/docs/'
    payload['README.md']=(f'Ripple {version} binary candidate for {target}\n\n'
        'Unzip and run ./ripple --help (Windows: ripple.exe). Add the directory to PATH if desired. Git supporting --no-lazy-fetch is required. No Rust, Node, Python or database server is required to run Ripple. SQLite is bundled; system runtime dependencies are listed in DYNAMIC-DEPENDENCIES.txt.\n\n'
        'In a supported TypeScript Git root: ripple index .; ripple status; ripple inspect FILE.ts; ripple impact FILE.ts::SYMBOL; ripple diff BASE..HEAD; ripple test BASE..HEAD. Use real existing refs/files/symbols. Test only reports potentially related files; no tests are executed and no set is sufficient. Unknown syntax/config/dynamic behavior remains incomplete; depth limits can omit tests.\n\n'
        'Archives are unsigned/not notarized. SHA256SUMS and manifest.json verify integrity, not authorship. Read BUILD.json for exact source/host/tool versions and THIRD-PARTY.json for notice scope. Source-relative links in SOURCE-README.md assume a source checkout.\n\n'
        'CLI documentation: '+docs+'cli.md\nLimitations: '+docs+'limitations.md\nEvaluation: '+docs+'evaluation.md\n'
        'Project license: LICENSE (MIT); upstream notices retain their own terms. Public release/tag publication is separate from this candidate.\n').encode()
    vendor=json.loads((ROOT/'docs/third-party-notices/manifest.json').read_text())['notices']
    notices=[]
    lock_checks={}
    for block in (ROOT/'Cargo.lock').read_text().split('[[package]]')[1:]:
        values={key:re.search(r'^'+key+r' = "([^"]+)"$',block,re.M) for key in ['name','version','checksum']}
        if all(values.values()):lock_checks[(values['name'][1],values['version'][1])]=values['checksum'][1]
    for package in metadata['packages']:
        if not package['source']:continue
        source=Path(package['manifest_path']).parent
        expected_package=lock_checks[(package['name'],package['version'])]
        cache=source.parents[2]/'cache'/source.parent.name/(package['name']+'-'+package['version']+'.crate')
        if digest(cache.read_bytes())!=expected_package:raise ValueError('locked crate archive checksum mismatch')
        files=[p for p in source.iterdir()if p.is_file()and p.name.upper().startswith(('LICENSE','COPYING','NOTICE','UNLICENSE','AUTHORS'))]
        entry={'name':package['name'],'version':package['version'],'license':package['license'],'cargo_checksum':expected_package,'notice_files':[]}
        for file in sorted(files):
            raw=file.read_bytes()
            with tarfile.open(cache,'r:gz')as archive:
                member=archive.getmember(package['name']+'-'+package['version']+'/'+file.name)
                if not member.isfile() or archive.extractfile(member).read()!=raw:raise ValueError('registry notice differs from locked archive')
            dest='third-party/'+package['name']+'-'+package['version']+'/'+file.name
            payload[dest]=raw;entry['notice_files'].append(dest)
        if not files:
            item=next((v for v in vendor if v['package']==package['name']+'-'+package['version']),None)
            if not item:raise ValueError('missing dependency notice: '+package['name'])
            raw=(ROOT/'docs/third-party-notices'/item['file']).read_bytes()
            if digest(raw)!=item['sha256']:raise ValueError('vendored notice mismatch')
            dest='third-party/'+item['file'];payload[dest]=raw;entry.update(notice_files=[dest],notice_provenance=item)
        notices.append(entry)
    payload['THIRD-PARTY.json']=(json.dumps({'scope':'Resolved Cargo.lock graph, including build/dev/other-target packages; not a precise linked-code SBOM. Upstream notices retain their own licenses.', 'packages':notices},indent=2)+'\n').encode()
    sysroot=Path(command(['rustc','--print','sysroot']).strip())
    for name in ['COPYRIGHT','LICENSE-MIT','LICENSE-APACHE']:
        file=sysroot/'share/doc/rust'/name
        if file.is_file():payload['toolchain/'+name]=file.read_bytes()
    if platform.system()=='Darwin':dependency_tool=['otool','-L',binary]
    elif platform.system()=='Linux':dependency_tool=['ldd',binary]
    else:
        vswhere=Path(os.environ.get('ProgramFiles(x86)','C:/Program Files (x86)'))/'Microsoft Visual Studio/Installer/vswhere.exe'
        install=command([vswhere,'-latest','-property','installationPath']).strip()
        dumpbins=sorted(Path(install).glob('VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe'))
        if not dumpbins:raise ValueError('dumpbin required for Windows dependency audit')
        dependency_tool=[dumpbins[-1],'/DEPENDENTS',binary]
    dependencies=command(dependency_tool).replace(str(binary),'ripple')
    if any(s in dependencies.lower() for s in ['libsqlite','sqlite3.dll']):raise ValueError('unexpected dynamic SQLite dependency')
    payload['DYNAMIC-DEPENDENCIES.txt']=dependencies.encode()
    build={'version':version,'target':target,'source_commit':source_commit,'cargo_lock_sha256':digest((ROOT/'Cargo.lock').read_bytes()),'rustc':command(['rustc','-vV']).strip(),'cargo':command(['cargo','--version']).strip(),'git':command(['git','--version']).strip(),'os':platform.platform(),'rustflags':os.environ.get('RUSTFLAGS',''),'sqlite':'bundled static C library; no separately installed SQLite library/server','unsigned':True,'public_release_created':False}
    payload['BUILD.json']=(json.dumps(build,indent=2)+'\n').encode()
    name='ripple-'+version+'-'+target+'.zip'
    output.mkdir(parents=True)
    archive=output/name
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED)as z:
        for path,raw in sorted(payload.items()):
            safe(path);info=zipfile.ZipInfo(path);info.compress_type=zipfile.ZIP_DEFLATED
            info.external_attr=(0o100755 if path==binary.name else 0o100644)<<16
            z.writestr(info,raw)
    manifest={'format_version':1,**build,'archive':name,'archive_sha256':digest(archive.read_bytes()),'binary':binary.name,'files':{p:digest(b)for p,b in sorted(payload.items())},'cargo_notice_packages':len(notices)}
    (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    (output/'SHA256SUMS').write_text(manifest['archive_sha256']+'  '+name+'\n')
    print(json.dumps({'prepared':name,'sha256':manifest['archive_sha256'],'notice_packages':len(notices)}))

def verify(directory,smoke=False):
    manifest=json.loads((directory/'manifest.json').read_text())
    safe(manifest['archive'])
    if len(PurePosixPath(manifest['archive']).parts)!=1:raise ValueError('archive name must be a basename')
    archive=directory/manifest['archive']
    if digest(archive.read_bytes())!=manifest['archive_sha256']:raise ValueError('archive checksum mismatch')
    if manifest['binary'] not in ('ripple','ripple.exe') or manifest['binary'] not in manifest['files']:raise ValueError('invalid binary name')
    with tempfile.TemporaryDirectory(prefix='ripple-unpacked-')as temp:
        root=Path(temp)
        with zipfile.ZipFile(archive)as z:
            if len(z.infolist())>4096 or len(z.namelist())!=len({n.casefold() for n in z.namelist()}):raise ValueError('duplicate/oversized inventory')
            if set(z.namelist())!=set(manifest['files']):raise ValueError('archive inventory mismatch')
            if sum(i.file_size for i in z.infolist())>128*1024*1024:raise ValueError('archive byte limit')
            for info in z.infolist():
                safe(info.filename)
                if stat.S_ISLNK(info.external_attr>>16)or info.file_size>64*1024*1024:raise ValueError('unsafe member')
                raw=z.read(info)
                if digest(raw)!=manifest['files'][info.filename]:raise ValueError('member checksum mismatch')
                file=root/info.filename;file.parent.mkdir(parents=True,exist_ok=True);file.write_bytes(raw)
                if info.filename==manifest['binary']:file.chmod(0o755)
        if smoke:
            binary=root/manifest['binary']
            if command([binary,'--version']).strip()!='ripple '+manifest['version']:raise ValueError('unpacked version mismatch')
            for script in ['scripts/m7-smoke.py','docs/examples/quickstart.py']:
                print(command([__import__('sys').executable,ROOT/script,'--binary',binary]).strip())
    print(json.dumps({'verified':manifest['archive'],'source_commit':manifest['source_commit'],'smoke':smoke}))
    return manifest

def main():
    p=argparse.ArgumentParser(description=__doc__);sub=p.add_subparsers(dest='action',required=True)
    a=sub.add_parser('prepare');a.add_argument('--binary',type=Path,required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--target',required=True)
    b=sub.add_parser('verify');b.add_argument('--directory',type=Path,required=True);b.add_argument('--smoke',action='store_true')
    args=p.parse_args()
    if args.action=='prepare':prepare(args.binary,args.output,args.target)
    else:verify(args.directory,args.smoke)
if __name__=='__main__':main()
