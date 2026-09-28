#!/usr/bin/env python3
"""Color settings hot reload in a real PTY; fixtures only, no native agent calls."""
import fcntl,json,os,select,struct,subprocess,sys,tempfile,termios,time,uuid
from pathlib import Path
ap=Path(sys.argv[1] if len(sys.argv)>1 else 'target/release/ap').resolve()
def drain(fd,seconds):
    data=b'';end=time.monotonic()+seconds
    while time.monotonic()<end:
        if select.select([fd],[],[],.025)[0]:data+=os.read(fd,65536)
    return data
with tempfile.TemporaryDirectory(prefix='ap-presentation-') as tmp:
    root=Path(tmp);source=root/'source.jsonl'
    source.write_text(json.dumps({'type':'session_meta','payload':{'id':str(uuid.uuid4()),'cwd':tmp}})+'\n'+json.dumps({'type':'event_msg','payload':{'type':'plan_update','plan':[{'step':'한글 현재 작업','status':'in_progress'}]}})+'\n')
    original=source.read_bytes();master,slave=os.openpty();baseline=termios.tcgetattr(slave)
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',5,80,0,0))
    env={**os.environ,'TERM':'xterm-256color','COLORTERM':'truecolor','HERDR_ENV':'0'};env.pop('NO_COLOR',None)
    child=subprocess.Popen([str(ap),'follow','--rollout',str(source),'--codex-home',tmp],stdin=slave,stdout=slave,stderr=slave,env=env)
    try:
        first=drain(master,.8);assert b'38;2;199;249;108' in first,first.decode(errors='replace')
        subprocess.run([str(ap),'config','set','--accent','#FF8899','--track','#112233'],cwd=root,check=True,stdout=subprocess.DEVNULL)
        changed=drain(master,1.2);assert b'38;2;255;136;153' in changed and b'48;2;17;34;51' in changed
        assert child.poll() is None and source.read_bytes()==original
        (root/'.agent-progress/ui.json').write_text('{')
        broken=drain(master,1.2);assert child.poll() is None
        subprocess.run([str(ap),'config','reset'],cwd=root,check=True,stdout=subprocess.DEVNULL)
        reset=drain(master,1.2);assert b'38;2;199;249;108' in reset
        subprocess.run([str(ap),'config','init-yaml'],cwd=root,check=True,stdout=subprocess.DEVNULL)
        yaml=root/'.agent-progress/ui.yaml'
        yaml.write_text("schema: 1\npreset: forest\nbackground_brightness: 1.5\nstates:\n  working:\n    track: '#203040'\n    accent: '#AABBCC'\n")
        themed=drain(master,1.2)
        assert b'48;2;48;72;96' in themed and b'38;2;170;187;204' in themed,themed.decode(errors='replace')
        yaml.write_text('states: [')
        drain(master,1.2);assert child.poll() is None and source.read_bytes()==original
        subprocess.run([str(ap),'config','reset'],cwd=root,check=True,stdout=subprocess.DEVNULL)
        restored=drain(master,1.2);assert b'38;2;199;249;108' in restored
        os.write(master,b'q');child.wait(timeout=3)
        assert child.returncode==0 and termios.tcgetattr(slave)==baseline
    finally:
        if child.poll() is None:child.kill();child.wait()
        os.close(master);os.close(slave)
print(json.dumps({'result':'passed','colors_hot_reload':True,'invalid_settings_keep_last_palette':True,'reset_restores_signal':True,'source_unchanged':True,'terminal_restored':True}))
