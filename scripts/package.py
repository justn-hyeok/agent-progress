#!/usr/bin/env python3
"""Build an unsigned, checksummed local macOS arm64 package without publishing."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import tempfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=root / 'dist')
args = parser.parse_args()
assert platform.system() == 'Darwin' and platform.machine() == 'arm64', 'only this host target is supported'
subprocess.run(['cargo', 'build', '--release', '--locked'], cwd=root, check=True)
binary = root / 'target/release/ap'
version = subprocess.check_output([str(binary), '--version'], text=True).split()[1]
name = f'agent-progress-{version}-macos-arm64'
libraries = subprocess.check_output(['otool', '-L', str(binary)], text=True)
assert all('/usr/lib/' in line or '/System/Library/' in line for line in libraries.splitlines()[1:]), libraries
args.output.mkdir(parents=True, exist_ok=True)
destination = args.output / f'{name}.tar.gz'
assert not destination.exists(), 'refusing to overwrite a package'
members = {
    'ap': binary.read_bytes(),
    'VERSION': (version + '\n').encode(),
    'install.sh': (root / 'scripts/install.sh').read_bytes(),
    'README.md': (root / 'docs/distribution.md').read_bytes(),
    'OPERATIONS.md': (root / 'docs/operations.md').read_bytes(),
    'CHANGELOG.md': (root / 'CHANGELOG.md').read_bytes(),
    'THIRD_PARTY.md': (root / 'docs/third-party.md').read_bytes(),
    'theme-example.yaml': (root / 'docs/theme-example.yaml').read_bytes(),
}
for skill_name in ('ap', 'ap-connect', 'ap-theme', 'ap-recover'):
    skill_dir = root / 'skills' / skill_name
    for relative in ('SKILL.md', 'agents/openai.yaml'):
        path = skill_dir / relative
        members[path.relative_to(root).as_posix()] = path.read_bytes()
    if skill_name == 'ap':
        for path in sorted((skill_dir / 'references').glob('*.md')):
            members[path.relative_to(root).as_posix()] = path.read_bytes()
members['SHA256SUMS'] = ''.join(f'{hashlib.sha256(data).hexdigest()}  {path}\n' for path, data in sorted(members.items())).encode()
with tempfile.NamedTemporaryFile(dir=args.output, delete=False) as temp:
    staged = Path(temp.name)
try:
    with staged.open('wb') as stream, gzip.GzipFile(fileobj=stream, mode='wb', mtime=0, filename='') as compressed, tarfile.open(fileobj=compressed, mode='w') as archive:
        for path, data in sorted(members.items()):
            info = tarfile.TarInfo(f'{name}/{path}')
            info.size = len(data)
            info.mode = 0o755 if path in ('ap', 'install.sh') else 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(data))
    os.link(staged, destination)
finally:
    staged.unlink(missing_ok=True)
digest = hashlib.sha256(destination.read_bytes()).hexdigest()
destination.with_suffix(destination.suffix + '.sha256').write_text(f'{digest}  {destination.name}\n')
print(json.dumps({'package':str(destination),'sha256':digest,'developer_signed':False,'notarized':False,'host':platform.platform(),'libraries':libraries.splitlines()[1:]}))
