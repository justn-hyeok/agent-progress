#!/usr/bin/env python3
"""Real tmux/PTY lifecycle, fixture native producer; no personal tmux server or credentials."""
import fcntl, json, os, select, socket, struct, subprocess, sys, tempfile, termios, threading, time, uuid
from pathlib import Path

ap = Path(sys.argv[1] if len(sys.argv)>1 else 'target/debug/ap').resolve()
masters=[]
frames={}
def until(predicate, timeout=12):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        for fd in masters:
            if select.select([fd],[],[],0)[0]:
                try:frames[fd]=(frames.get(fd,b'')+os.read(fd,65536))[-8000:]
                except OSError:pass
        result=predicate()
        if result:return result
        time.sleep(.05)
    raise AssertionError('terminal scenario timed out')

with tempfile.TemporaryDirectory(prefix='ap-terminal-test-') as tmp:
    root=Path(tmp); bin=root/'bin';bin.mkdir()
    (root/'ap.project.json').write_text(json.dumps({'schema':1,'project_id':str(uuid.uuid4()),'objective':'Portable fixture','roadmap':'plan.md'}))
    (root/'plan.md').write_text('- [ ] AP-01 Observer\n- [ ] AP-02 Isolation\n')
    native=bin/'claude'
    native.write_text('''#!/usr/bin/env python3
import json,os,sys,subprocess,time
from pathlib import Path
if sys.argv[1:]==['--version']:print('fixture 1');sys.exit()
if sys.argv[1:]==['--help']:print('--settings --output-format --resume');sys.exit()
slot=Path(os.environ['AP_TERMINAL_SLOT']); label=sys.argv[1]
root=Path.cwd(); (root/(label+'.slot')).write_text(str(slot))
(root/(label+'.args')).write_text(json.dumps(sys.argv[1:]))
event={'cwd':str(root),'session_id':label,'todos':[{'content':'AP-01 '+label,'status':'in_progress'}]}
subprocess.run([os.environ['AP_TEST_BIN'],'bridge','--agent','claude'],input=json.dumps(event),text=True,check=True,stdout=subprocess.DEVNULL)
resized=False
while not (root/(label+'.end')).exists():
 if label=='first' and not resized and (root/'first.resize').exists():
  subprocess.run([os.environ['AP_TEST_BIN'],'config','set','--pane-size','40'],cwd=root,check=True,stdout=subprocess.DEVNULL)
  (root/'first.resized').write_text('done')
  resized=True
 time.sleep(.05)
sys.exit(9 if label=='fourth' else 0)
''')
    native.chmod(0o755)
    # Actual Unix/WebSocket transport through the Codex proxy; only the producer is a fixture.
    codex=bin/'codex'
    codex.write_text('''#!/usr/bin/env python3
import json,os,sys,socket,struct,subprocess,time
from pathlib import Path
a=sys.argv[1:]
if a==['--version']:print('fixture 1');sys.exit()
if a==['--help']:print('--remote resume app-server');sys.exit()
if '--config' in a:print(json.dumps({'native_args':a,'terminal_slot':'AP_TERMINAL_SLOT' in os.environ}));sys.exit(5)
if a[:2]==['app-server','generate-json-schema']:
 p=Path(a[a.index('--out')+1])/'v2';p.mkdir()
 for n in ['ThreadStartResponse','ThreadResumeResponse','ThreadForkResponse']:(p/(n+'.json')).write_text(json.dumps({'thread':{'cwd':{},'id':{},'path':{}}}))
 sys.exit()
root=Path.cwd();slot=Path(os.environ['AP_TERMINAL_SLOT']);(root/'third.slot').write_text(str(slot))
s=socket.socket(socket.AF_UNIX);s.connect(a[a.index('--remote')+1].removeprefix('unix://'))
s.sendall(b'GET / HTTP/1.1\\r\\nUpgrade: websocket\\r\\n\\r\\n')
header=b''
while not header.endswith(b'\\r\\n\\r\\n'):header+=s.recv(1)
data=json.dumps({'id':1,'method':'thread/start','params':{}}).encode();mask=b'abcd'
s.sendall(bytes([0x81,0x80|len(data)])+mask+bytes(v^mask[i%4] for i,v in enumerate(data)))
resized=False
while not (root/'third.end').exists():
 if not resized and (root/'third.resize').exists():
  subprocess.run([os.environ['AP_TEST_BIN'],'config','set','--pane-size','20'],cwd=root,check=True,stdout=subprocess.DEVNULL)
  (root/'third.resized').write_text('done')
  resized=True
 time.sleep(.05)
s.close()
sys.exit(7)
''')
    # Python 3.9's removesuffix/removeprefix is available on the macOS CI runner.
    codex.chmod(0o755)
    backend=root/'backend.sock'; listener=socket.socket(socket.AF_UNIX);listener.bind(str(backend));listener.listen()
    def backend_server():
        connection,_=listener.accept()
        with connection:
            header=b''
            while not header.endswith(b'\r\n\r\n'):header+=connection.recv(1)
            connection.sendall(b'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n')
            def read(count):
                data=b''
                while len(data)<count:data+=connection.recv(count-len(data))
                return data
            frame=read(2);mask=read(4);payload=read(frame[1]&127)
            request=json.loads(bytes(v^mask[i%4] for i,v in enumerate(payload)))
            assert request['method']=='thread/start'
            session=str(uuid.uuid4());rollout=root/'native.jsonl'
            rollout.write_text(json.dumps({'type':'session_meta','payload':{'id':session,'cwd':str(root)}})+'\n'+json.dumps({'type':'event_msg','payload':{'type':'plan_update','plan':[{'step':'AP-01 third','status':'in_progress'}]}})+'\n')
            data=json.dumps({'id':1,'result':{'thread':{'id':session,'path':str(rollout),'cwd':str(root),'ephemeral':False}}}).encode()
            connection.sendall(bytes([0x81,126])+struct.pack('!H',len(data))+data)
            while connection.recv(1024):pass
    thread=threading.Thread(target=backend_server,daemon=True);thread.start()
    children=[]; handles=[]; slots=[]
    env={**os.environ,'HERDR_ENV':'0','TERM':'xterm-256color','AP_TEST_BIN':str(ap),'PATH':str(bin)+':'+os.environ['PATH']}
    for key in ['TMUX','TMUX_PANE','AP_TERMINAL_SLOT']:env.pop(key,None)
    try:
        for label in ['first','second','third']:
            if label=='third':subprocess.run([str(ap),'config','set','--position','above','--pane-size','40'],cwd=root,env=env,check=True,stdout=subprocess.DEVNULL)
            master,slave=os.openpty();handles.extend([master,slave])
            masters.append(master)
            fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,120,0,0))
            command=[str(ap),'launch','--agent','claude','--',label,"quoted a'b",'literal$HOME',''] if label!='third' else [str(ap),'launch','--agent','codex','--','--remote','unix://'+str(backend),label]
            child=subprocess.Popen(command,stdin=slave,stdout=slave,stderr=slave,cwd=root,env=env)
            children.append(child)
            until(lambda:(root/(label+'.slot')).exists())
            slot=Path((root/(label+'.slot')).read_text());slots.append(slot)
            until(lambda:(slot/'selection.json').exists())
            selection=json.loads((slot/'selection.json').read_text())
            assert selection['owner']==int((slot/'owner').read_text())
            if label!='third':assert json.loads((root/(label+'.args')).read_text())==[label,"quoted a'b",'literal$HOME','']
            def call(*args):
                return subprocess.run(['tmux','-S',str(slot/'tmux.sock'),*args],check=True,capture_output=True,text=True).stdout
            panes=call('list-panes','-t','ap','-F','#{pane_id} #{pane_active} #{pane_height}').splitlines()
            assert len(panes)==2,panes
            source=next(p.split()[0] for p in panes if p.split()[1]=='1')
            observer=next(p.split()[0] for p in panes if p.split()[1]=='0')
            window_height=int(call('display-message','-p','-t',source,'#{window_height}'))
            expected_rows=(window_height*(40 if label=='third' else 30)+50)//100
            assert int(next(p.split()[2] for p in panes if p.split()[1]=='0'))==expected_rows,panes
            if label=='first':
                (root/'first.resize').touch()
                until(lambda:(root/'first.resized').exists())
                resized_panes=call('list-panes','-t','ap','-F','#{pane_id} #{pane_active} #{pane_height}').splitlines()
                assert int(next(p.split()[2] for p in resized_panes if p.split()[1]=='0'))==(window_height*40+50)//100,resized_panes
                assert next(p.split()[0] for p in resized_panes if p.split()[1]=='1')==source
                subprocess.run([str(ap),'config','set','--pane-size','30'],cwd=root,env=env,check=True,stdout=subprocess.DEVNULL)
            if label=='third':
                (root/'third.resize').touch()
                until(lambda:(root/'third.resized').exists())
                resized_panes=call('list-panes','-t','ap','-F','#{pane_id} #{pane_active} #{pane_height}').splitlines()
                assert int(next(p.split()[2] for p in resized_panes if p.split()[1]=='0'))==(window_height*20+50)//100,resized_panes
                assert resized_panes[0].split()[0]==observer
                assert next(p.split()[0] for p in resized_panes if p.split()[1]=='1')==source
            assert panes[0].split()[1]==('0' if label=='third' else '1'),panes
            until(lambda:label in call('capture-pane','-p','-t',observer))
            # Actual pane resize keeps the observer alive and its source unchanged.
            call('resize-window','-t','ap','-x','90','-y','24')
            until(lambda:label in call('capture-pane','-p','-t',observer))
            snapshot=json.loads(subprocess.check_output([str(ap),'terminal-follow','--slot',str(slot),'--once'],env=env,text=True))
            assert snapshot['session']==selection['session']
        assert json.loads((slots[0]/'selection.json').read_text())['session']!=json.loads((slots[1]/'selection.json').read_text())['session']
        for label in ['first','second','third']:(root/(label+'.end')).touch()
        for child,expected in zip(children,[0,0,7]):until(lambda:child.poll() is not None);assert child.returncode==expected,(child.returncode,expected)
        assert all(not slot.exists() for slot in slots)
        master,slave=os.openpty();handles.extend([master,slave]);masters.append(master)
        child=subprocess.Popen([str(ap),'launch','--agent','codex','--','--config','user.setting=true'],stdin=slave,stdout=slave,stderr=slave,cwd=root,env=env);children.append(child)
        until(lambda:child.poll() is not None);assert child.returncode==5
        until(lambda:b'native_args' in frames.get(master,b''))
        assert b'"terminal_slot": false' in frames[master]
        # Existing tmux: preserve an unrelated window and return native failure to the caller.
        existing=root/'existing.sock'
        (root/'.agent-progress/ui.json').write_text('{')
        subprocess.run(['tmux','-S',str(existing),'-f','/dev/null','new-session','-d','-s','owned-fixture','-x','120','-y','40','sleep 300'],env=env,check=True)
        master,slave=os.openpty();handles.extend([master,slave]);masters.append(master)
        env_existing={**env,'TMUX':str(existing)+',0,0'}
        child=subprocess.Popen([str(ap),'launch','--agent','claude','--','fourth'],stdin=slave,stdout=slave,stderr=slave,cwd=root,env=env_existing);children.append(child)
        until(lambda:(root/'fourth.slot').exists())
        slot=Path((root/'fourth.slot').read_text());slots.append(slot)
        try:until(lambda:(slot/'selection.json').exists())
        except AssertionError:
            raise AssertionError({'fixture_outer_status':child.poll(),'slot_exists':slot.exists(),'output':frames.get(master,b'').decode(errors='replace')[-2000:]})
        (root/'fourth.end').touch()
        until(lambda:child.poll() is not None);assert child.returncode==9,child.returncode
        until(lambda:not slot.exists())
        windows=subprocess.check_output(['tmux','-S',str(existing),'list-windows','-F','#{window_panes}'],text=True).splitlines()
        assert windows==['1'],windows
        subprocess.run(['tmux','-S',str(existing),'kill-server'],check=True)
    finally:
        for slot in slots:subprocess.run(['tmux','-S',str(slot/'tmux.sock'),'kill-server'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        if (root/'existing.sock').exists():subprocess.run(['tmux','-S',str(root/'existing.sock'),'kill-server'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        for child in children:
            if child.poll() is None:child.kill();child.wait()
        for fd in handles:os.close(fd)
        listener.close();thread.join(timeout=1)
print(json.dumps({'result':'passed','real_tmux':True,'producer':'fixture','exact_sessions':4,'codex_transport':'unix/websocket','quoting':True,'automatic_display':True,'source_focus':True,'resize':True,'source_exit_cleanup':True,'native_exit_status':True,'existing_tmux_preserved':True}))
