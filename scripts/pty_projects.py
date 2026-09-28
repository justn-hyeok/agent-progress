#!/usr/bin/env python3
"""Read-only product switching preserves each source's data and restores the terminal."""
import json
import fcntl
import os
from pathlib import Path
import re
import select
import subprocess
import struct
import sys
import tempfile
import termios
import time
import uuid

binary=Path(sys.argv[1] if len(sys.argv)>1 else 'target/release/ap').resolve()
def expect(fd,child,text):
    seen=b'';end=time.monotonic()+5
    while time.monotonic()<end and child.poll() is None:
        if select.select([fd],[],[],.05)[0]:seen+=os.read(fd,65536)
        if text in re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',seen.decode(errors='replace')):return
    raise AssertionError(f'missing {text}')
with tempfile.TemporaryDirectory(prefix='ap-projects-pty-') as tmp:
    root=Path(tmp);manifests=[];states=[]
    for i,goal in enumerate(['AAAAAAAAAAAAA','ZZZZZZZZZZZZZ']):
        folder=root/str(i);folder.mkdir();project=str(uuid.uuid4());session=str(uuid.uuid4())
        manifest=folder/'ap.project.json';manifest.write_text(json.dumps({'schema':1,'project_id':project,'objective':goal,'roadmap':'plan.md'}));manifests.append(manifest)
        (folder/'plan.md').write_text('- [ ] AP-01 Work\n')
        source=folder/'source.jsonl';source.write_text(json.dumps({'type':'session_meta','payload':{'id':session,'cwd':str(folder)}})+'\n'+json.dumps({'type':'event_msg','payload':{'type':'plan_update','plan':[{'step':'AP-01 Work','status':'completed' if i==0 else 'pending'}]}})+'\n')
        subprocess.run([str(binary),'follow','--rollout',str(source),'--codex-home',str(folder),'--once'],check=True,stdout=subprocess.DEVNULL)
        state=folder/'.agent-progress'/f'project-{project}.json';states.append((state,state.read_bytes()))
    master,slave=os.openpty();baseline=termios.tcgetattr(slave)
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',25,100,0,0))
    child=subprocess.Popen([str(binary),'projects','--project',*[str(p) for p in manifests]],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'})
    try:
        expect(master,child,'AAAAAAAAAAAAA');os.write(master,b'\t');expect(master,child,'ZZZZZZZZZZZZZ')
        os.write(master,b'\x1b[Z');expect(master,child,'AAAAAAAAAAAAA')
        os.write(master,b'q');child.wait(timeout=3)
        assert child.returncode==0 and termios.tcgetattr(slave)==baseline
        assert all(path.read_bytes()==before for path,before in states)
    finally:
        if child.poll() is None:child.kill();child.wait()
        os.close(master);os.close(slave)
print(json.dumps({'result':'passed','switch_and_return':True,'product_states_unchanged':True,'terminal_restored':True}))
