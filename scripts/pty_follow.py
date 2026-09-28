#!/usr/bin/env python3
"""Synthetic Codex stream -> real ap follow PTY -> durable progress + clean exit."""
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


def drain(fd, seconds=0.6):
    output = b""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if select.select([fd], [], [], 0.03)[0]:
            output += os.read(fd, 65536)
    return output


def glyphs(data):
    return re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", data.decode(errors="replace")).replace(" ", "")


def write_event(path, value):
    with path.open("a") as f:
        f.write(json.dumps(value, ensure_ascii=False) + "\n")


def plan(path, status):
    write_event(path, {"type": "event_msg", "timestamp": "2026-09-24T11:22:33Z",
                      "payload": {"type": "plan_update", "plan": [
                          {"step": "세션에서 자동으로 가져온 첫 작업", "status": status},
                          {"step": "긴 한국어 줄바꿈과 실제 상태 반영 검증", "status": "pending"}]}})


with tempfile.TemporaryDirectory(prefix="ap-follow-") as tmp:
    root = Path(tmp)
    source = root / "rollout.jsonl"
    cache = root / "progress.json"
    write_event(source, {"type": "session_meta", "payload": {"id": str(uuid.uuid4()), "cwd": tmp}})
    plan(source, "in_progress")
    for exit_key in [b"q", b"\x03"]:
        master, slave = os.openpty()
        baseline = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 14, 80, 0, 0))
        child = subprocess.Popen([str(binary), "follow", "--rollout", str(source),
                                  "--cache", str(cache), "--codex-home", tmp],
                                 stdin=slave, stdout=slave, stderr=slave,
                                 env={**os.environ, "TERM": "xterm-256color"})
        try:
            first = b""
            ready_deadline = time.monotonic() + 5
            while "체크리스트" not in glyphs(first) and child.poll() is None and time.monotonic() < ready_deadline:
                first += drain(master, 0.05)
            assert child.poll() is None, first.decode(errors="replace")
            assert b"\x1b[?1049h" in first
            assert "체크리스트" in glyphs(first)
            assert "세션에서자동으로가져온첫작업" in glyphs(first)
            os.write(master, b"\r")
            assert "에이전트보고" in glyphs(drain(master))
            os.write(master, b"\r")
            drain(master)
            plan(source, "completed")
            drain(master)
            saved = json.loads(cache.read_text())["snapshot"]
            assert [e["state"] for e in saved["entries"]] == ["Done", "Pending"]
            original_size = source.stat().st_size
            with source.open("a") as f:
                f.write("{malformed}\n")
            assert "오래된상태" in glyphs(drain(master))
            with source.open("r+") as f:
                f.truncate(original_size)
            plan(source, "in_progress")
            drain(master)
            saved = json.loads(cache.read_text())["snapshot"]
            assert saved["entries"][0]["state"] == "Active"
            assert saved["entries"][0]["was_done"]
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 40, 0, 0))
            drain(master)
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 5, 40, 0, 0))
            drain(master)
            os.write(master, b"?")
            assert "도움말" in glyphs(drain(master))
            os.write(master, b"\x1b")
            drain(master)
            os.write(master, b"\r")
            assert "항목상세" in glyphs(drain(master))
            os.write(master, b"\x1b")
            drain(master)
            os.write(master, exit_key)
            child.wait(timeout=4)
            output = drain(master, 0.1)
            assert child.returncode == 0, output.decode(errors="replace")
            assert b"\x1b[?1049l" in output
            assert termios.tcgetattr(slave) == baseline
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=3)
            os.close(master)
            os.close(slave)
    print(json.dumps({"result": "passed", "source": "synthetic Codex JSONL",
                      "checks": ["automatic checklist", "live state update", "history after reopening",
                                 "malformed source + recovery", "80x14 to 40x12 to 40x5", "compact help/detail", "q/Ctrl-C terminal restoration"]}))
