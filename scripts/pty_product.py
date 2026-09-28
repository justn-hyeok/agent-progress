#!/usr/bin/env python3
"""Product percentage and current subtask must stay separate in the real TUI."""
import fcntl
import json
import os
from pathlib import Path
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
import uuid

binary=Path(sys.argv[1] if len(sys.argv)>1 else 'target/debug/ap').resolve()
def append(path,payload):
    with path.open('a') as f:f.write(json.dumps(payload,ensure_ascii=False)+'\n')
def plan(path,text):
    stamp=int(time.time()*1000)
    append(path,{'type':'event_msg','payload':{'type':'item_completed','completed_at_ms':stamp,'item':{'type':'Plan','text':text}}})
def drain(fd,seconds=.6):
    data=b'';end=time.monotonic()+seconds
    while time.monotonic()<end:
        if select.select([fd],[],[],.02)[0]:data+=os.read(fd,65536)
    return data
def glyphs(data):
    return re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',data.decode(errors='replace')).replace(' ','')

with tempfile.TemporaryDirectory(prefix='ap-product-pty-') as tmp:
    root=Path(tmp);pid=str(uuid.uuid4());sid=str(uuid.uuid4());source=root/'rollout.jsonl'
    (root/'ap.project.json').write_text(json.dumps({'schema':1,'project_id':pid,'objective':'제품 전체 연결','roadmap':'product.md'}))
    (root/'product.md').write_text('- [ ] ROOT-01 First acceptance\n- [ ] ROOT-02 Second acceptance\n')
    append(source,{'type':'session_meta','payload':{'id':sid,'cwd':tmp}})
    plan(source,'- [x] ROOT-01 First acceptance\n- [>] [ROOT-02/bridge] 현재 세부 작업')
    master,slave=os.openpty();baseline=termios.tcgetattr(slave)
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',5,100,0,0))
    child=subprocess.Popen([str(binary),'follow','--rollout',str(source),'--codex-home',tmp],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'})
    state=root/'.agent-progress'/f'project-{pid}.json'
    try:
        seen=b'';until=time.monotonic()+5
        while '현재세부작업' not in glyphs(seen) and child.poll() is None and time.monotonic()<until:seen+=drain(master,.05)
        assert child.poll() is None,seen.decode(errors='replace')
        assert '제품전체연결' in glyphs(seen) and '50%' in glyphs(seen) and '현재세부작업' in glyphs(seen)
        plan(source,'- [x] [ROOT-02/bridge] 현재 세부 작업')
        drain(master,.8)
        saved=json.loads(state.read_text())['plan']['tasks']
        assert [x['status'] for x in saved]==['done','review'],saved
        os.write(master,b'\r');detail=glyphs(drain(master));
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',30,120,0,0));detail+=glyphs(drain(master))
        assert '제품전체' in detail and '1/2' in detail
        os.write(master,b'\x1b');drain(master)
        plan(source,'- [x] ROOT-02 Second acceptance')
        drain(master,.8)
        assert all(t['status']=='done' for t in json.loads(state.read_text())['plan']['tasks'])
        os.write(master,b'q');child.wait(timeout=3)
        assert child.returncode==0 and termios.tcgetattr(slave)==baseline
    finally:
        if child.poll() is None:child.kill();child.wait(timeout=3)
        os.close(master);os.close(slave)
    print(json.dumps({'result':'passed','product_percentage':'50% while subplan was 100%','root_acceptance':'explicit root completion required','current_subtask':'shown','detail_and_resize':'passed','terminal_restored':True}))
