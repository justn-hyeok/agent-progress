#!/usr/bin/env python3
"""First-run pane sizing through a real PTY and the standalone config CLI."""
import json
import os
import pty
import select
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ap = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/ap').resolve()
with tempfile.TemporaryDirectory(prefix='ap-setup-test-') as directory:
    root = Path(directory)
    (root / 'ap.project.json').write_text(json.dumps({
        'schema': 1, 'project_id': 'c38c35ef-108f-4ba3-9dc1-69390819b21e',
        'objective': 'Sizing fixture', 'roadmap': 'plan.md',
    }))
    (root / 'plan.md').write_text('- [ ] AP-01 Existing work\n')
    env = {**os.environ, 'HERDR_ENV': '0', 'HOME': str(root / 'home')}
    env.pop('AP_TERMINAL_SLOT', None)
    (root / 'home').mkdir()

    def command(*args):
        return subprocess.run([str(ap), *args], cwd=root, env=env, capture_output=True, text=True)

    assert not (root / '.agent-progress').exists()
    assert command('setup').returncode != 0, 'non-terminal setup must explain the CLI alternative'
    assert not (root / '.agent-progress').exists()

    def setup(answer):
        master, slave = pty.openpty()
        process = subprocess.Popen([str(ap), 'setup'], cwd=root, env=env,
                                   stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        os.write(master, answer.encode() + b'\n')
        deadline = time.monotonic() + 8
        output = b''
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                try:
                    output += os.read(master, 4096)
                except OSError:
                    break
            if process.poll() is not None:
                break
        status = process.wait(timeout=1)
        os.close(master)
        assert status == 0, output.decode(errors='replace')
        assert '진행 창 높이' in output.decode(errors='replace')

    settings_path = root / '.agent-progress/ui.json'
    setup('1')
    assert json.loads(settings_path.read_text())['pane_size_percent'] == 10
    setup('4')
    assert json.loads(settings_path.read_text())['pane_size_percent'] == 40
    assert command('config', 'set', '--pane-size', '25').returncode == 0
    before = settings_path.read_bytes()
    assert json.loads(before)['pane_size_percent'] == 25
    assert command('config', 'set', '--pane-size', '9').returncode != 0
    assert settings_path.read_bytes() == before
    assert command('config', 'set', '--preset', 'forest', '--position', 'above').returncode == 0
    setup('')
    retained = json.loads(settings_path.read_text())
    assert retained['pane_size_percent'] == 25
    assert retained['preset'] == 'forest' and retained['position'] == 'above'
    assert command('config', 'init-yaml').returncode == 0
    assert command('config', 'set', '--pane-size', '20').returncode == 0
    assert json.loads(command('config', 'show').stdout)['pane_size_percent'] == 20
    assert command('config', 'reset').returncode == 0
    assert json.loads(command('config', 'show').stdout)['pane_size_percent'] == 10

print(json.dumps({'result': 'passed', 'interactive_setup': True,
                  'project_local': True, 'yaml': True, 'invalid_preserved': True}))
