#!/usr/bin/env python3
"""Install a built archive into a temporary prefix: upgrade from --previous, rollback,
re-upgrade, exercise the CLI and uninstall, keeping project data and unrelated files."""
import argparse
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('archive', type=Path)
parser.add_argument('--previous', type=Path, help='an older release archive to upgrade from and roll back to')
options = parser.parse_args()
env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_', 'TMUX', 'CODEX_', 'AP_'))}
env['AP_AUTO_OPEN'] = '0'


def run(*args, cwd=None):
    return subprocess.check_output([str(a) for a in args], text=True, cwd=cwd, env=env)


def unpack(archive, into):
    with tarfile.open(archive.resolve()) as stream:
        for member in stream.getmembers():
            assert member.isfile() and not member.name.startswith('/') and '..' not in Path(member.name).parts
        stream.extractall(into, filter='data')
    return next(into.iterdir())


with tempfile.TemporaryDirectory(prefix='ap-package-smoke-') as tmp:
    temp = Path(tmp)
    package = unpack(options.archive, temp / 'new')
    assert (package / 'skills/ap/SKILL.md').is_file()
    prefix = temp / 'prefix'
    ap = prefix / 'bin/ap'
    (prefix / 'bin').mkdir(parents=True)
    unrelated = prefix / 'bin/unrelated-tool'
    unrelated.write_text('keep me\n')
    old_version = None
    if options.previous:
        previous = unpack(options.previous, temp / 'old')
        run('sh', previous / 'install.sh', '--prefix', prefix)
        old_version = run(ap, '--version').split()[-1]
    run('sh', package / 'install.sh', '--prefix', prefix)
    version = run(ap, '--version').split()[-1]
    assert package.name == f'agent-progress-{version}-macos-arm64', (package.name, version)
    assert version != old_version
    project = temp / 'project'
    project.mkdir()
    run(ap, 'goal', '패키지 확인', cwd=project)
    run(ap, 'add', '하나', '둘', cwd=project)
    run(ap, 'done', '1', cwd=project)
    assert '패키지 확인 · 1/2 (50%)' in run(ap, 'status', cwd=project)
    plan = project / '.agent-progress/plans/default.json'
    saved = plan.read_bytes()
    if old_version:
        run('sh', package / 'install.sh', '--prefix', prefix, '--rollback')
        assert run(ap, '--version').split()[-1] == old_version
        assert '1/2 (50%)' in run(ap, 'status', cwd=project), 'previous release must read the plan'
        assert plan.read_bytes() == saved
        run('sh', package / 'install.sh', '--prefix', prefix)
        assert run(ap, '--version').split()[-1] == version
    run('sh', package / 'install.sh', '--prefix', prefix, '--uninstall')
    assert not ap.exists() and not ap.is_symlink()
    assert unrelated.read_text() == 'keep me\n'
    assert plan.read_bytes() == saved
    print(f'package smoke passed: {version}' + (f' (upgrade/rollback from {old_version})' if old_version else ''))
