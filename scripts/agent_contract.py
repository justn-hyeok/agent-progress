#!/usr/bin/env python3
"""Actual Claude/OpenCode plan producers. No delegated coding, shell tools or user/global config edits."""
import argparse, json, os, subprocess, time, uuid
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('agent',choices=['claude','opencode'])
parser.add_argument('--binary',type=Path,default=ROOT/'target/release/ap')
parser.add_argument('--native-tasks',action='store_true')
parser.add_argument('--artifact',action='store_true',help='Verify native file-write work in this private fixture only')
parser.add_argument('--model',help='Explicit per-run native model; does not edit user settings')
parser.add_argument('--without-product',action='store_true',help='Exercise passive hooks without an ap.project.json declaration')
args=parser.parse_args(); ap=args.binary.resolve()
folder=ROOT/'.agent-progress/multi-agent-contract'/str(uuid.uuid4())
folder.mkdir(parents=True)
os.chmod(folder,0o700)
pid=str(uuid.uuid4()); session=str(uuid.uuid4())
if not args.without_product:
    (folder/'ap.project.json').write_text(json.dumps({'schema':1,'project_id':pid,'objective':'Actual native adapter contract','roadmap':'plan.md'}))
    (folder/'plan.md').write_text('- [ ] AP-01 Observe native input\n- [ ] AP-02 Preserve across restart\n')
env={**os.environ,'HERDR_ENV':'0'}
env.pop('AP_TERMINAL_SLOT',None)
env['AP_AUTO_OPEN']='0'
subprocess.run([str(ap),'connect','apply','--agent',args.agent],cwd=folder,env=env,check=True,stdout=subprocess.DEVNULL)
if args.agent=='opencode':
    permission={'*':'deny','todowrite':'allow','todoread':'allow'}
    if args.artifact:
        for tool in ['edit','write','read']:
            target=folder/'native-result.txt'
            permission[tool]={'*':'deny','native-result.txt':'allow',str(target):'allow',str(target).lstrip('/'):'allow',str(target.relative_to(ROOT)):'allow'}
    (folder/'opencode.json').write_text(json.dumps({'permission':permission}))

def producer(prompt,resume=None):
    if args.agent=='claude':
        tools='TaskCreate,TaskUpdate' if args.native_tasks else ''
        if args.artifact:tools+=',Write'
        command=['claude','--print','--output-format','json','--tools',tools, '--model','haiku',
                 '--setting-sources','project,local','--settings',str(folder/'.claude/settings.local.json')]
        if args.native_tasks: command+=['--restricted','--strict-mcp-config','--mcp-config','{"mcpServers":{}}']
        if args.artifact:command+=['--allowedTools','Write,TaskCreate,TaskUpdate']
        command+=['--resume',resume] if resume else ['--session-id',session]
    else:
        command=['opencode','run','--print-logs','--log-level','ERROR','--format','json','--dir',str(folder)]
        if args.model:command+=['--model',args.model]
        if resume:command+=['--session',resume]
    command.append(prompt)
    result=subprocess.run(command,cwd=folder,env=env,capture_output=True,text=True,timeout=120)
    private=folder/f'producer-{len(list(folder.glob("producer-*.json")))}.json'
    private.write_text(result.stdout); os.chmod(private,0o600)
    log=private.with_suffix('.log');log.write_text(result.stderr);os.chmod(log,0o600)
    if result.returncode:
        raise RuntimeError(f'{args.agent} producer failed ({result.returncode}); private log: {log}')
    if args.agent=='claude':
        output=json.loads(result.stdout)
        if output.get('is_error'):raise RuntimeError(f'Claude producer error; private log: {log}')
        return output['session_id']
    for line in result.stdout.splitlines():
        try:event=json.loads(line)
        except ValueError:continue
        if event.get('sessionID'):return event['sessionID']
    raise RuntimeError('OpenCode returned no exact session ID')

def counts():
    if args.without_product:
        entries=snapshot()['entries']
        return [sum(item['state']=='Done' for item in entries),len(entries)]
    return json.loads(subprocess.check_output([str(ap),'product','resume'],cwd=folder,env=env,text=True))['counts']

def snapshot():
    streams=list((folder/'.agent-progress/bridges').glob(f'{args.agent}-*.jsonl'))
    assert len(streams)==1,streams
    observed=json.loads(subprocess.check_output([str(ap),'follow','--rollout',str(streams[0]),'--once','--codex-home',str(folder)],cwd=folder,env=env,text=True))
    if args.without_product:
        assert observed.get('overall') is None
        assert not (folder/'ap.project.json').exists()
    return observed

intro='This is only a native progress adapter test. No coding, no filesystem or shell work, no subagents. Any checklist output must start with the exact heading "### 진행 계획". '
if args.artifact:intro='This is only a native tool/adapter QA fixture, no production coding, no shell or subagents. The only file you may change is '+str(folder/'native-result.txt')+'. Any checklist output must start with the exact heading "### 진행 계획". '
if args.agent=='claude':
    prompt=intro+'Output exactly these two Markdown checklist rows: - [ ] AP-01 Observe native input\n- [ ] AP-02 Preserve across restart'
    if args.native_tasks:prompt=intro+'Use TaskCreate to create exactly two pending tasks with subjects "[AP-01] Observe native input" and "[AP-02] Preserve across restart". Then reply only READY, no checklist.'
    s=producer(prompt)
else:
    s=producer(intro+'Call todowrite once with exactly two pending todos: "AP-01 Observe native input" and "AP-02 Preserve across restart". Then output exactly those two Markdown checklist rows, both pending.')
assert counts()==[0,2],counts()
progress=(intro+'Output exactly these two rows: - [x] AP-01 Observe native input\n- [>] AP-02 Preserve across restart')
if args.agent=='opencode':progress=intro+'Call todowrite with the first todo completed and second in_progress. Keep the same two titles. Then output the matching two Markdown checklist rows.'
if args.agent=='claude' and args.native_tasks:progress=intro+'Use TaskUpdate to mark task 1 completed and task 2 in_progress. Then reply only READY, no checklist.'
if args.artifact:progress=intro+'First use the native Write tool to write exactly "native input observed\\n" (one line with a newline, without quotes) to '+str(folder/'native-result.txt')+'. After the write succeeds, '+progress[len(intro):]
s=producer(progress,s); assert counts()==[1,2],counts()
if args.artifact:assert (folder/'native-result.txt').read_text()=='native input observed\n'
state=folder/'.agent-progress'/f'project-{pid}.json'
ids=[item['id'] for item in snapshot()['entries']] if args.without_product else [t['id'] for t in json.loads(state.read_text())['plan']['tasks']]
# New process, same explicit native session; adapter state and product IDs must survive.
finish=intro+'Output exactly these two rows: - [x] AP-01 Observe native input\n- [x] AP-02 Preserve across restart'
if args.agent=='opencode':finish=intro+'Call todowrite with both existing todos completed. Keep both titles. Then output their completed Markdown checklist rows.'
if args.agent=='claude' and args.native_tasks:finish=intro+'Use TaskUpdate to mark task 2 completed. Then reply only READY, no checklist.'
if args.artifact:finish=intro+'First use the native Write tool to replace '+str(folder/'native-result.txt')+' with exactly two lines: "native input observed" then "resumed in new process", each ending with a newline. After the write succeeds, '+finish[len(intro):]
s=producer(finish,s); assert counts()==[2,2],counts()
if args.artifact:assert (folder/'native-result.txt').read_text()=='native input observed\nresumed in new process\n'
assert ([item['id'] for item in snapshot()['entries']] if args.without_product else [t['id'] for t in json.loads(state.read_text())['plan']['tasks']])==ids
streams=list((folder/'.agent-progress/bridges').glob('*.jsonl'))
assert len(streams)==1,streams
observed=snapshot()
assert args.agent in observed['source'],observed['source']
print(json.dumps({'result':'passed','agent':args.agent,'version':subprocess.check_output([args.agent,'--version'],text=True).strip(),'model':args.model or ('haiku' if args.agent=='claude' else 'host default'),'session':s,'project':str(folder),'counts':[0,1,2],'stable_ids':True,'actual_hook_or_plugin':True,'resume_in_new_process':True,'native_fixture_file_work':args.artifact,'without_product':args.without_product,'stream':str(streams[0]),'source':observed['source'],'ap_version':subprocess.check_output([str(ap),'--version'],text=True).strip(),'ap_binary_sha256':__import__('hashlib').sha256(ap.read_bytes()).hexdigest()},ensure_ascii=False))
