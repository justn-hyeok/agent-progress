#!/usr/bin/env python3
"""Real Codex app-server producer smoke. No edits/delegated coding; private test threads only."""
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("AP_BINARY", ROOT / "target/debug/ap")).resolve()
EVIDENCE = ROOT / ".agent-progress/native-contract"
EVIDENCE.mkdir(parents=True, exist_ok=True)


class Client:
    def __init__(self):
        self.events = []
        self.queue = queue.Queue()
        self.seq = 0
        self.stderr = (EVIDENCE / "server-private.log").open("w")
        os.chmod(EVIDENCE / "server-private.log", 0o600)
        self.process = subprocess.Popen(["codex", "app-server", "--stdio"], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.stderr, text=True, bufsize=1)
        def reader():
            for line in self.process.stdout:
                try:
                    self.queue.put(json.loads(line))
                except ValueError:
                    pass
        threading.Thread(target=reader, daemon=True).start()
        self.call("initialize", {"clientInfo": {"name": "agent-progress-contract-test", "version": "0.3.0"},
                                  "capabilities": {"experimentalApi": True}})
        self.send({"method": "initialized"})

    def send(self, value):
        self.process.stdin.write(json.dumps(value) + "\n")
        self.process.stdin.flush()

    def receive(self, timeout=60):
        value = self.queue.get(timeout=timeout)
        self.events.append(value)
        if value.get("method") and "id" in value:
            # The producer is not allowed to run commands, write files or request input.
            self.send({"id": value["id"], "error": {"code": -32000, "message": "No tool execution approved in producer contract test"}})
        return value

    def call(self, method, params, timeout=45):
        self.seq += 1
        request = self.seq
        self.send({"id": request, "method": method, "params": params})
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = self.receive(max(0.1, deadline - time.monotonic()))
            if value.get("id") == request and not value.get("method"):
                if "error" in value:
                    raise RuntimeError(f"{method}: {value['error'].get('message', 'failed')}")
                return value.get("result")
        raise TimeoutError(method)

    def turn(self, thread, text, mode="plan"):
        result = self.call("turn/start", {"threadId": thread, "input": [{"type": "text", "text": text}],
                                         "collaborationMode": {"mode": mode, "settings": {"model": "gpt-6-sol", "reasoning_effort": "low", "developer_instructions": None}}})
        turn = result["turn"]["id"]
        deadline = time.monotonic() + 100
        while time.monotonic() < deadline:
            event = self.receive(max(0.1, deadline - time.monotonic()))
            if event.get("method") == "turn/completed" and event.get("params", {}).get("turn", {}).get("id") == turn:
                assert event["params"]["turn"]["status"] == "completed", "native turn did not complete"
                return
        self.call("turn/interrupt", {"threadId": thread, "turnId": turn})
        raise TimeoutError("native producer turn")

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=4)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=4)
        self.stderr.close()


if __name__ == "__main__":
    test_dir = EVIDENCE / "runs" / str(uuid.uuid4())
    test_dir.mkdir(parents=True)
    product_id=str(uuid.uuid4())
    (test_dir/'ap.project.json').write_text(json.dumps({"schema":1,"project_id":product_id,"objective":"Native integration contract","roadmap":"product.md"}))
    (test_dir/'product.md').write_text("# Native integration contract\n- [ ] NATIVE-01 Observe native input\n- [ ] NATIVE-02 Preserve state across sessions\n")
    def read_progress(path):
        return json.loads(subprocess.check_output([str(BINARY),'follow','--rollout',path,'--once'],text=True))
    def counts(snapshot):
        return [sum(e['state']=='Done' for e in snapshot['entries']),len(snapshot['entries'])]
    start_params={
        "cwd":str(test_dir),"approvalPolicy":"never","sandbox":"read-only","model":"gpt-6-sol",
        "baseInstructions":"You are a goal/plan event producer for a local application contract test. Do not inspect files, run commands, delegate, or perform coding work. Only emit the requested native plan or explicit execution status, then stop. Do not modify goals.",
        "developerInstructions":"Parent executes all checks. You only format its stated status exactly as requested. Never use tools or ask questions."}
    client = Client()
    try:
        started = client.call("thread/start", start_params)
        thread = started["thread"]["id"]
        goal = client.call("thread/goal/set", {"threadId": thread, "objective": "agent-progress isolated native input contract", "status": "paused"})
        client.turn(thread, "This is a planning-format contract test, no actual work to execute. Output a final proposed_plan with heading 'Native contract' and exactly these two checklist rows: '- [ ] NATIVE-01 Observe native input' and '- [ ] NATIVE-02 Preserve state across sessions'. Do not use tools or ask questions.")
        read = client.call("thread/read", {"threadId": thread, "includeTurns": False})
        path=read['thread']['path']
        initial=read_progress(path)
        assert initial['overall']['session_goal']['objective']=='agent-progress isolated native input contract'
        assert 'native Plan' in initial['source'] and counts(initial)==[0,2]
        client.turn(thread,"Parent has successfully observed native goal and Plan input. In normal execution mode, output exactly this plain Markdown status, not a proposed_plan: '### 진행 계획\n목표: agent-progress isolated native input contract\n- [x] NATIVE-01 Observe native input\n- [>] NATIVE-02 Preserve state across sessions'. No tools, no extra work.",mode="default")
        partial=read_progress(path)
        assert counts(partial)==[1,2]
        assert '진행 보고' in partial['source'], 'execution report did not advance native plan'
        client.call('thread/goal/set',{'threadId':thread,'status':'complete'})
        assert counts(read_progress(path))==[1,2], 'session goal completion became whole-product completion'
        second=client.call('thread/start',start_params)['thread']['id']
        client.call('thread/goal/set',{'threadId':second,'objective':'Continue native persistence verification','status':'paused'})
        client.turn(second,"Output proposed_plan with one row '- [>] NATIVE-02 Preserve state across sessions'. Parent handles all work; do not add other rows or use tools.")
        path2=client.call('thread/read',{'threadId':second,'includeTurns':False})['thread']['path']
        resumed=read_progress(path2)
        assert counts(resumed)==[1,2] and resumed['goal']['id']==product_id
        assert [e['id'] for e in partial['entries']]==[e['id'] for e in resumed['entries']]
        client.turn(second,"Parent verified that the second session retained task IDs and the first completed item. Output proposed_plan with only '- [x] NATIVE-02 Preserve state across sessions'. No tools, no further work.")
        completed=read_progress(path2)
        assert counts(completed)==[2,2]
        assert counts(read_progress(path))==[2,2], 'stale first session regressed product state'
        assert counts(read_progress(path2))==[2,2], 'fresh reader failed to resume'
        native = [e["params"] for e in client.events if e.get("method") == "turn/plan/updated" or (e.get("method") == "item/completed" and e.get("params",{}).get("item",{}).get("type") == "plan")]
        client.call('thread/goal/set',{'threadId':second,'objective':'Native goal lifecycle change verified','status':'paused'})
        changed=read_progress(path2)
        assert counts(changed)==[2,2] and changed['overall']['session_goal']['objective']=='Native goal lifecycle change verified'
        client.call('thread/goal/set',{'threadId':second,'status':'complete'})
        client.close()
        client=Client()
        persisted=client.call('thread/goal/get',{'threadId':second})
        assert persisted['goal']['status']=='complete'
        assert counts(read_progress(path2))==[2,2], 'app-server restart lost product state'
        public = {"threads": [thread,second], "paths": [path,path2], "project_id":product_id,"goal": goal, "native_plan_updates": native,
                  "counts":[counts(initial),counts(partial),counts(resumed),counts(completed)],"stable_ids":True,"stale_session_preserved":True,"goal_lifecycle_and_restart":True,"native_plan_to_execution_report":True,
                  "version": subprocess.check_output(["codex", "--version"], text=True).strip(), "model": "gpt-6-sol", "mode":"native Plan, default host configuration"}
        public["ap_version"] = subprocess.check_output([str(BINARY),"--version"],text=True).strip()
        public["ap_binary_sha256"] = __import__('hashlib').sha256(BINARY.read_bytes()).hexdigest()
        (test_dir / "native-result.json").write_text(json.dumps(public, ensure_ascii=False, indent=2))
        (EVIDENCE / "native-result.json").write_text(json.dumps(public, ensure_ascii=False, indent=2))
        print(json.dumps(public, ensure_ascii=False), flush=True)
        assert native, "native plan event was not emitted"
    finally:
        client.close()
