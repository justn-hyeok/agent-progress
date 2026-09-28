#!/usr/bin/env python3
"""Real Codex lifecycle/MCP probe in an owned Herdr fixture, with no model coding."""
import json,os,shlex,subprocess,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
folder=ROOT/'.agent-progress/codex-hook-contract'/str(uuid.uuid4());folder.mkdir(parents=True)
pid=str(uuid.uuid4())
(folder/'ap.project.json').write_text(json.dumps({'schema':1,'project_id':pid,'objective':'Native hook and MCP contract','roadmap':'plan.md'}))
(folder/'plan.md').write_text('- [ ] AP-01 Observe\n- [ ] AP-02 Preserve\n')
ap=ROOT/'target/release/ap'
subprocess.run([str(ap),'connect','apply'],cwd=folder,env={**os.environ,'HERDR_ENV':'0'},check=True,stdout=subprocess.DEVNULL)
command=shlex.join(['codex','exec','--skip-git-repo-check','--ignore-user-config','--dangerously-bypass-hook-trust',
    '--disable','plugins','-C',str(folder),'-m','gpt-6-sol','-c','model_reasoning_effort="low"','--json',
    'This is only a native hook/MCP contract. Do not run shell, change files, or use subagents. Call the agent_progress plan_read MCP tool once, then output exactly: ### 진행 계획\n- [x] AP-01 Observe\n- [>] AP-02 Preserve'])
(folder/'launch.sh').write_text('#!/bin/sh\ncd '+shlex.quote(str(folder))+'\n'+command+' > '+shlex.quote(str(folder/'producer.jsonl'))+' 2> '+shlex.quote(str(folder/'producer.log'))+'\n')
print(json.dumps({'folder':str(folder),'launch':str(folder/'launch.sh'),'project_id':pid}))
