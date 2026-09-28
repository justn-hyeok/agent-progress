#!/usr/bin/env python3
"""Review regressions: hung source probe and recoverable checkpoint write failure."""
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

binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/ap").resolve()


def drain(fd, seconds=0.5):
    data = b""
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if select.select([fd], [], [], 0.02)[0]:
            data += os.read(fd, 65536)
    return data


def text(data):
    return re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", data.decode(errors="replace")).replace(" ", "")


def append(path, value):
    with path.open("a") as file:
        file.write(json.dumps(value, ensure_ascii=False) + "\n")


def plan(path, state):
    append(path, {"type": "event_msg", "payload": {"type": "plan_update", "plan": [
        {"step": "검토용 실제 터미널 계획", "status": state}]}})


def launch(args, env):
    master, slave = os.openpty()
    baseline = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 14, 80, 0, 0))
    child = subprocess.Popen([str(binary), *args], stdin=slave, stdout=slave, stderr=slave,
                             env={**os.environ, **env, "TERM": "xterm-256color"})
    data = b""
    ready_deadline = time.monotonic() + 5
    while b"\x1b[?1049h" not in data and child.poll() is None and time.monotonic() < ready_deadline:
        data += drain(master, 0.05)
    if child.poll() is not None or b"\x1b[?1049h" not in data:
        child.kill() if child.poll() is None else None
        child.wait(timeout=3)
        os.close(master)
        os.close(slave)
        raise AssertionError(data.decode(errors="replace"))
    return child, master, slave, baseline


def finish(child, master, slave, baseline, deadline=2):
    started = time.monotonic()
    os.write(master, b"q")
    child.wait(timeout=deadline)
    elapsed = time.monotonic() - started
    assert child.returncode == 0, drain(master, 0.1).decode(errors="replace")
    assert termios.tcgetattr(slave) == baseline
    return elapsed


def cleanup(child, master, slave):
    if child.poll() is None:
        child.kill()
        child.wait(timeout=3)
    os.close(master)
    os.close(slave)


with tempfile.TemporaryDirectory(prefix="ap-review-") as tmp:
    root = Path(tmp)
    source = root / f"rollout-{uuid.uuid4()}.jsonl"
    append(source, {"type": "session_meta", "payload": {"id": str(uuid.uuid4()), "cwd": tmp}})
    plan(source, "in_progress")
    mockbin = root / "bin"
    mockbin.mkdir()
    helper = mockbin / "herdr"
    helper.write_text(f"#!{sys.executable}\n" + '''import os,sys,json,time
from pathlib import Path
a=sys.argv[1:]
if a==['agent','get','w1:p1']:
 if Path(os.environ['AP_DELAY']).exists():
  Path(os.environ['AP_HELPER_PID']).write_text(str(os.getpid()))
  time.sleep(20)
 result={'agent':{'agent':'codex','terminal_id':'fixed','agent_status':'working'}}
elif a==['pane','process-info','--pane','w1:p1']:
 result={'process_info':{'foreground_processes':[{'name':'codex','pid':123}]}}
else:sys.exit(2)
print(json.dumps({'result':result}))
''')
    helper.chmod(0o700)
    lsof = mockbin / "lsof"
    lsof.write_text(f"#!{sys.executable}\nimport os\nprint('n'+os.environ['AP_ROLLOUT'])\n")
    lsof.chmod(0o700)
    delay = root / "delay"
    helper_pid = root / "helper.pid"
    env = {"PATH": f"{mockbin}:{os.environ['PATH']}", "HERDR_ENV": "1",
           "AP_DELAY": str(delay), "AP_HELPER_PID": str(helper_pid), "AP_ROLLOUT": str(source)}
    child, master, slave, baseline = launch(["follow", "--pane", "w1:p1", "--codex-home", tmp,
                                           "--cache", str(root / "probe.json")], env)
    try:
        delay.touch()
        until = time.monotonic() + 4
        while not helper_pid.exists() and time.monotonic() < until:
            drain(master, 0.05)
        assert helper_pid.exists(), "slow Herdr probe was not exercised"
        elapsed = finish(child, master, slave, baseline, deadline=1)
        try:
            os.kill(int(helper_pid.read_text()), 0)
        except ProcessLookupError:
            pass
        else:
            raise AssertionError("cancelled helper was not reaped")
    finally:
        cleanup(child, master, slave)

    cache = root / "writable" / "progress.json"
    child, master, slave, baseline = launch(["follow", "--rollout", str(source), "--codex-home", tmp,
                                           "--cache", str(cache)], {})
    try:
        until = time.monotonic() + 2
        while not cache.is_file() and time.monotonic() < until:
            drain(master, 0.05)
        assert cache.is_file()
        retained = cache.with_suffix(".retained")
        cache.rename(retained)
        cache.mkdir()  # A directory blocks atomic checkpoint replacement.
        plan(source, "completed")
        assert "저장실패" in text(drain(master, 0.8))
        drain(master, 0.6)  # No new source change; the warning must remain pending.
        cache.rmdir()
        retained.rename(cache)
        drain(master, 1.5)
        assert json.loads(cache.read_text())["snapshot"]["entries"][0]["state"] == "Done"
        finish(child, master, slave, baseline)
    finally:
        cleanup(child, master, slave)
    print(json.dumps({"result": "passed", "hung_probe_exit_seconds": round(elapsed, 3),
                      "helper_reaped": True, "storage_retry_without_new_source_event": True}))
