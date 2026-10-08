#!/usr/bin/env python3
"""Render cell dumps from the `dump_previews` test into PNGs (visual check only)."""
import json, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cells_png import draw  # noqa: E402

for path in sorted(Path(sys.argv[1]).glob('*.json')):
    out = path.with_suffix('.png')
    draw(json.loads(path.read_text())).save(out)
    print(out)
