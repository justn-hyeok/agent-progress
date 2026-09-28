#!/usr/bin/env python3
"""Bounded real-terminal lifecycle test; only writes to a temporary fixture."""
import fcntl
import json
import os
import re
from pathlib import Path
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/ap").resolve()


def run(path, *args):
    return subprocess.check_output([str(binary), "--file", str(path), *args], text=True)


def drain(master, duration=0.5):
    chunks = []
    deadline = time.monotonic() + duration
    while time.monotonic() < deadline:
        if select.select([master], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
            chunks.append(os.read(master, 65536))
    return b"".join(chunks)


def text(data):
    # Ratatui can position each wide glyph separately; assertions examine
    # emitted glyphs, while TestBackend tests cover cell placement.
    return re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", data.decode(errors="replace")).replace(" ", "")


def check(path, quit_key):
    master, slave = os.openpty()
    baseline = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    process = subprocess.Popen([str(binary), "--file", str(path), "watch"],
                               stdin=slave, stdout=slave, stderr=slave,
                               env={**os.environ, "TERM": "xterm-256color"})
    try:
        rendered = drain(master, 0.8)
        assert process.poll() is None, rendered.decode(errors="replace")
        assert b"\x1b[?1049h" in rendered, "alternate screen not entered"
        assert not termios.tcgetattr(slave)[3] & termios.ICANON, "raw mode not entered"
        os.write(master, b"\r")
        details = drain(master)
        assert "완료조건" in text(details), "detail keyboard action failed"
        good = path.read_bytes()
        path.write_text("# interrupted editor save")
        stale = drain(master)
        assert "오래된상태" in text(stale), "stale data not labeled"
        path.write_bytes(good)
        drain(master)
        path.unlink()
        assert "오래된상태" in text(drain(master)), "deletion not labeled"
        path.write_bytes(good)
        drain(master)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 40, 0, 0))
        drain(master)
        os.write(master, quit_key)
        process.wait(timeout=3)
        exited = drain(master, 0.1)
        assert process.returncode == 0
        assert b"\x1b[?1049l" in exited, "alternate screen not restored"
        assert termios.tcgetattr(slave) == baseline, "terminal attributes not restored"
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=3)
        os.close(master)
        os.close(slave)


with tempfile.TemporaryDirectory(prefix="ap-pty-") as directory:
    path = Path(directory) / "plan.md"
    run(path, "init", "--project", "PTY fixture", "--goal", "한글 상태 확인")
    run(path, "add", "실제 터미널 복귀 확인", "--criterion", "종료 후 모드 복원",
        "--reason", "PTY 검증")
    for quit_key in (b"q", b"\x03"):
        check(path, quit_key)
    print(json.dumps({"result": "passed", "exits": ["q", "Ctrl-C"],
                      "viewports": ["80x24", "40x12"],
                      "checks": ["detail", "corrupt file", "deletion", "recovery", "resize", "termios restoration"]}))
