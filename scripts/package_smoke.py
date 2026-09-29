#!/usr/bin/env python3
"""Install actual ap releases in a temporary prefix, update/rollback and preserve project data."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import uuid

root=Path(__file__).resolve().parents[1]
archive=Path(sys.argv[1] if len(sys.argv)>1 else 'dist/agent-progress-0.4.0-macos-arm64.tar.gz').resolve()
def run(args,**kwargs):return subprocess.check_output([str(x) for x in args],text=True,**kwargs)
with tempfile.TemporaryDirectory(prefix='ap-package-smoke-') as tmp:
    temp=Path(tmp);unpacked=temp/'unpacked';unpacked.mkdir()
    with tarfile.open(archive) as stream:
        for member in stream.getmembers():
            assert member.isfile() and not member.name.startswith('/') and '..' not in Path(member.name).parts
        stream.extractall(unpacked)
    package=next(unpacked.iterdir());prefix=temp/'prefix'
    installer=package/'install.sh'
    run(['sh',installer,'--prefix',prefix])
    ap=prefix/'bin/ap';initial_version=run([ap,'--version']).strip()
    original_version=initial_version.split()[-1]
    # A real product is created from a source plan without init/add/UUID registration.
    project=temp/'project';project.mkdir();pid=str(uuid.uuid4())
    (project/'ap.project.json').write_text(json.dumps({'schema':1,'project_id':pid,'objective':'Installed product','roadmap':'plan.md'}))
    (project/'plan.md').write_text('- [ ] AP-01 First\n- [ ] AP-02 Second\n')
    source=project/'source.jsonl'
    source.write_text(json.dumps({'type':'session_meta','payload':{'id':str(uuid.uuid4()),'cwd':str(project)}})+'\n'+json.dumps({'type':'event_msg','timestamp':'2026-09-28T00:00:00Z','payload':{'type':'plan_update','plan':[{'step':'AP-01 First','status':'completed'}]}})+'\n')
    original=source.read_bytes()
    clean_env={**os.environ,'PATH':'/usr/bin:/bin:/usr/sbin:/sbin'}
    observed=json.loads(run([ap,'follow','--rollout',source,'--codex-home',project,'--once'],env=clean_env))
    assert observed['entries'][0]['state']=='Done'
    assert json.loads(run([ap,'product','resume'],cwd=project))['counts']==[1,2]
    state=project/'.agent-progress'/f'project-{pid}.json';before=state.read_bytes()
    preview=json.loads(run([ap,'connect','preview'],cwd=project))
    assert str(ap) in preview['files'][0]['managed_addition'], 'project adapters must follow the public update symlink'
    native=json.loads(run([sys.executable,root/'scripts/native_contract.py'],env={**os.environ,'AP_BINARY':str(ap)}))
    assert native['stable_ids'] and native['goal_lifecycle_and_restart'] and native['ap_version']==initial_version
    run_id=uuid.uuid4().hex[:8]
    native_result=root/'.agent-progress'/f'installed-native-result-{initial_version.split()[-1]}-{native["ap_binary_sha256"][:12]}-{run_id}.json'
    assert not native_result.exists(), 'preserve prior installed native evidence'
    native_result.write_text(json.dumps(native,ensure_ascii=False,indent=2))
    adapters={}
    for agent in ['claude','opencode']:
        args=[sys.executable,root/'scripts/agent_contract.py',agent,'--binary',ap,'--artifact']
        if agent=='claude': args+=['--native-tasks']
        else:args+=['--model','opencode-go/glm-5.3-flash']
        adapters[agent]=json.loads(run(args))
        assert adapters[agent]['ap_version']==initial_version
    adapter_result=root/'.agent-progress'/f'installed-adapters-{original_version}-{native["ap_binary_sha256"][:12]}-{run_id}.json'
    assert not adapter_result.exists(), 'preserve prior installed adapter evidence'
    adapter_result.write_text(json.dumps(adapters,ensure_ascii=False,indent=2))
    # Build a genuine ap update with the same source and schema, in an isolated fixture.
    build=temp/'update-source';build.mkdir()
    shutil.copytree(root/'src',build/'src')
    original_version=initial_version.split()[-1]
    major,minor,patch=original_version.split('.')
    version=f'{major}.{minor}.{int(patch)+1}-smoke'
    for name in ['Cargo.toml','Cargo.lock']:
        content=(root/name).read_text()
        if name=='Cargo.toml':content=content.replace(f'version = "{original_version}"',f'version = "{version}"',1)
        else:content=content.replace(f'name = "agent-progress"\nversion = "{original_version}"',f'name = "agent-progress"\nversion = "{version}"',1)
        (build/name).write_text(content)
    subprocess.run(['cargo','build','--release','--locked','--manifest-path',str(build/'Cargo.toml'),'--target-dir',str(root/'target/package-smoke')],check=True,stdout=subprocess.DEVNULL)
    update=temp/'update';shutil.copytree(package,update)
    shutil.copy2(root/'target/package-smoke/release/ap',update/'ap')
    (update/'VERSION').write_text(version+'\n')
    (update/'SHA256SUMS').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(update).as_posix()}\n' for p in sorted(update.rglob('*')) if p.is_file() and p.name!='SHA256SUMS'))
    run(['sh',update/'install.sh','--prefix',prefix])
    assert version in run([ap,'--version'])
    assert json.loads(run([ap,'product','resume'],cwd=project))['counts']==[1,2]
    assert state.read_bytes()==before
    run(['sh',update/'install.sh','--prefix',prefix,'--rollback'])
    assert run([ap,'--version']).strip()==initial_version
    assert json.loads(run([ap,'product','resume'],cwd=project))['counts']==[1,2]
    unrelated=prefix/'bin/keep-me';unrelated.write_text('user data')
    run(['sh',installer,'--prefix',prefix,'--uninstall'])
    assert not ap.exists() and not ap.is_symlink()
    assert unrelated.read_text()=='user data' and state.read_bytes()==before and source.read_bytes()==original
    # Reject an unrelated executable before changing it.
    ap.write_text('user-owned executable')
    result=subprocess.run(['sh',installer,'--prefix',str(prefix)],capture_output=True,text=True)
    assert result.returncode!=0 and ap.read_text()=='user-owned executable'
print(json.dumps({'result':'passed','installed':initial_version,'updated':version,'rollback':True,'uninstall':True,'product_data_preserved':True,'unrelated_files_preserved':True,'source_unchanged':True,'core_with_clean_path':True,'installed_native_codex':True,'installed_claude_and_opencode':True,'native_evidence':str(native_result),'adapter_evidence':str(adapter_result),'environment':'temporary prefix on current host; not a clean OS VM'}))
