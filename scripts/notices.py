#!/usr/bin/env python3
"""Collect locked dependency metadata and shipped license texts from local Cargo sources."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--filter-platform','aarch64-apple-darwin','--format-version','1'],cwd=root))
lines = ['# Third-party dependencies', '', 'Generated from Cargo.lock and local package metadata. License expressions are the package authors\' declarations.', '']
for package in sorted(metadata['packages'], key=lambda p: (p['name'],p['version'])):
    if package['name'] == 'agent-progress': continue
    lines.extend([f"## {package['name']} {package['version']}", '', f"License: {package['license'] or 'see package license file'}", f"Source: {package.get('repository') or package['source']}", ''])
    parent = Path(package['manifest_path']).parent
    files = sorted({*parent.glob('LICENSE*'),*parent.glob('COPYING*'),*parent.glob('NOTICE*')})
    if package.get('license_file'): files.append(parent / package['license_file'])
    for path in dict.fromkeys(files):
        if path.is_file():
            lines.extend([f'### {path.name}', '', '```text',path.read_text(errors='replace').strip(),'```',''])
(root / 'docs/third-party.md').write_text('\n'.join(lines))
print(json.dumps({'packages':len(metadata['packages'])-1,'notice':'docs/third-party.md'}))
