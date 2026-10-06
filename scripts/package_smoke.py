#!/usr/bin/env python3
"""Install a built archive into a temporary prefix, exercise the CLI, uninstall, keep project data."""
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

archive = Path(sys.argv[1]).resolve()
env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_', 'TMUX', 'CODEX_', 'AP_'))}
env['AP_AUTO_OPEN'] = '0'

def run(*args, cwd=None):
    return subprocess.check_output([str(a) for a in args], text=True, cwd=cwd, env=env)

with tempfile.TemporaryDirectory(prefix='ap-package-smoke-') as tmp:
    temp = Path(tmp)
    with tarfile.open(archive) as stream:
        for member in stream.getmembers():
            assert member.isfile() and not member.name.startswith('/') and '..' not in Path(member.name).parts
        stream.extractall(temp / 'unpacked', filter='data')
    package = next((temp / 'unpacked').iterdir())
    assert (package / 'skills/ap/SKILL.md').is_file()
    prefix = temp / 'prefix'
    run('sh', package / 'install.sh', '--prefix', prefix)
    ap = prefix / 'bin/ap'
    version = run(ap, '--version').split()[-1]
    assert package.name == f'agent-progress-{version}-macos-arm64', (package.name, version)
    project = temp / 'project'
    project.mkdir()
    run(ap, 'goal', '패키지 확인', cwd=project)
    run(ap, 'add', '하나', '둘', cwd=project)
    run(ap, 'done', '1', cwd=project)
    status = run(ap, 'status', cwd=project)
    assert '패키지 확인 · 1/2 (50%)' in status, status
    run('sh', package / 'install.sh', '--prefix', prefix, '--uninstall')
    assert not ap.exists()
    assert (project / '.agent-progress/plans/default.json').is_file()
    print(f'package smoke passed: {version}')
