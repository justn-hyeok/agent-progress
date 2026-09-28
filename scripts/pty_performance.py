#!/usr/bin/env python3
"""Measure actual PTY incremental updates for 10/100/500 MiB sources, not fixture timing guesses."""
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
results=[]
def drain(fd):
    data=b''
    while select.select([fd],[],[],.02)[0]: data+=os.read(fd,65536)
    return data
def wait_text(fd,child,needle,timeout):
    start=time.monotonic(); seen=b''
    while time.monotonic()-start<timeout and child.poll() is None:
        seen+=drain(fd)
        if needle in re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',seen.decode(errors='replace')): return time.monotonic()-start
    raise AssertionError(f'missing {needle!r}; exit={child.poll()}')
for size in [10,100,500]:
    with tempfile.TemporaryDirectory(prefix='ap-perf-') as tmp:
        source=Path(tmp)/'source.jsonl'
        with source.open('w') as stream:
            stream.write(json.dumps({'type':'session_meta','payload':{'id':str(uuid.uuid4()),'cwd':tmp}})+'\n')
            ignored=json.dumps({'type':'ignored','payload':'x'*(1024*1024)})+'\n'
            for _ in range(size):stream.write(ignored)
            stream.write(json.dumps({'type':'event_msg','payload':{'type':'plan_update','plan':[{'step':'AAAAAAAAAAAAA','status':'in_progress'}]}})+'\n')
        master,slave=os.openpty(); baseline=termios.tcgetattr(slave)
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',5,100,0,0))
        start=time.monotonic()
        child=subprocess.Popen([str(binary),'follow','--rollout',str(source),'--codex-home',tmp],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'TERM':'xterm-256color'})
        try:
            wait_text(master,child,'AAAAAAAAAAAAA',60); initial=time.monotonic()-start
            drain(master)
            # Disjoint equal-width strings make every cell change; terminal diff rendering
            # otherwise emits only changed cells and a flat ANSI strip can miss the update.
            with source.open('a') as stream:stream.write(json.dumps({'type':'event_msg','payload':{'type':'plan_update','plan':[{'step':'ZZZZZZZZZZZZZ','status':'in_progress'}]}})+'\n')
            latency=wait_text(master,child,'ZZZZZZZZZZZZZ',10)
            start=time.monotonic();os.write(master,b'q');child.wait(timeout=2);exit_time=time.monotonic()-start
            assert child.returncode==0 and termios.tcgetattr(slave)==baseline
            results.append({'size_mib':size,'initial_seconds':round(initial,3),'incremental_seconds':round(latency,3),'exit_seconds':round(exit_time,3),'incremental_under_2s':latency<2,'exit_under_1s':exit_time<1})
        finally:
            if child.poll() is None:child.kill();child.wait()
            os.close(master);os.close(slave)
print(json.dumps({'runtime':'real PTY, synthetic Codex JSONL','results':results}))
assert all(r['incremental_under_2s'] and r['exit_under_1s'] for r in results)
