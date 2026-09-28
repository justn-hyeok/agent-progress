#!/usr/bin/env python3
"""Abrupt exit at checkpoint write boundaries, using a disposable PTY/cache/source."""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
import uuid

binary=Path(sys.argv[1] if len(sys.argv)>1 else 'target/debug/ap').resolve()
def append(path,status):
    with path.open('a') as stream:stream.write(json.dumps({'type':'event_msg','payload':{'type':'plan_update','plan':[{'step':'checkpoint work','status':status}]}})+'\n')
def wait(child,fd,predicate,limit=5):
    end=time.monotonic()+limit
    while time.monotonic()<end and not predicate():
        if select.select([fd],[],[],.05)[0]:os.read(fd,65536)
    assert predicate(), 'checkpoint stage not reached'
results=[]
for stage in ['checkpoint:temp-synced','checkpoint:replaced','checkpoint:dir-synced']:
    with tempfile.TemporaryDirectory(prefix='ap-checkpoint-fault-') as tmp:
        root=Path(tmp);source=root/'source.jsonl';cache=root/'cache.json'
        source.write_text(json.dumps({'type':'session_meta','payload':{'id':str(uuid.uuid4()),'cwd':tmp}})+'\n')
        append(source,'pending')
        def launch(fault=None):
            master,slave=os.openpty()
            env={**os.environ,'TERM':'xterm-256color'}
            if fault:env['AP_FAULT_STAGE']=fault
            child=subprocess.Popen([str(binary),'follow','--rollout',str(source),'--cache',str(cache),'--codex-home',tmp],stdin=slave,stdout=slave,stderr=slave,env=env)
            return child,master,slave
        child,master,slave=launch()
        try:
            wait(child,master,cache.exists)
            os.write(master,b'q');child.wait(timeout=3)
            assert child.returncode==0
        finally:
            if child.poll() is None:child.kill();child.wait()
            os.close(master);os.close(slave)
        old=cache.read_bytes();append(source,'completed')
        child,master,slave=launch(stage)
        try:
            wait(child,master,lambda:child.poll() is not None)
            assert child.returncode==86
            saved=json.loads(cache.read_text())
            assert saved['snapshot']['entries'][0]['state']==('Pending' if stage.endswith('temp-synced') else 'Done')
            if stage.endswith('temp-synced'):assert cache.read_bytes()==old
        finally:
            if child.poll() is None:child.kill();child.wait()
            os.close(master);os.close(slave)
        # A normal restart must accept the saved checksum/prefix and the newer source.
        child,master,slave=launch()
        try:
            wait(child,master,lambda:json.loads(cache.read_text())['snapshot']['entries'][0]['state']=='Done')
            os.write(master,b'q');child.wait(timeout=3);assert child.returncode==0
        finally:
            if child.poll() is None:child.kill();child.wait()
            os.close(master);os.close(slave)
        results.append({'stage':stage,'consistent':True,'restart_recovered':True})
print(json.dumps({'result':'passed','abrupt_exit_without_unwinding':results}))
